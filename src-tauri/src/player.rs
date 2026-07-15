use crate::error::{AppError, AppResult};
use rodio::{OutputStream, Sink};
use std::io::Read;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};
use tauri::Emitter;
use crate::models::PlayerCmd;

fn get_ffmpeg_path() -> String {
    if Path::new("./ffmpeg.exe").exists() { "./ffmpeg.exe".to_string() } 
    else if Path::new("../ffmpeg.exe").exists() { "../ffmpeg.exe".to_string() } 
    else if Path::new("./ffmpeg").exists() { "./ffmpeg".to_string() } 
    else if Path::new("../ffmpeg").exists() { "../ffmpeg".to_string() } 
    else { "ffmpeg".to_string() }
}

// 📍 重构 1：将音频设备初始化抽离为标准函数
fn init_audio_device(target_device: Option<&str>) -> AppResult<(OutputStream, Sink)> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let host = rodio::cpal::default_host();
    let mut device_opt = None;
    
    if let Some(dev_name) = target_device {
        if let Ok(mut devices) = host.output_devices() {
            device_opt = devices.find(|d| d.name().unwrap_or_default() == dev_name);
        }
    }
    
    let device = device_opt
        .or_else(|| host.default_output_device())
        .ok_or_else(|| AppError::Audio("系统中未找到音频输出设备".into()))?;

    let (stream, stream_handle) = OutputStream::try_from_device(&device)
        .map_err(|e| AppError::Audio(format!("音频流建立失败: {}", e)))?;
    let sink = Sink::try_new(&stream_handle)
        .map_err(|e| AppError::Audio(format!("Sink 建立失败: {}", e)))?;

    Ok((stream, sink))
}

// 📍 重构 2：将 FFmpeg 重启逻辑抽离为标准函数
fn restart_ffmpeg(
    start_secs: f64,
    file_path: &str,
    ffmpeg_process: &mut Option<Child>,
) -> AppResult<ChildStdout> {
    if let Some(mut old) = ffmpeg_process.take() { 
        let _ = old.kill(); 
    }
    
    let mut cmd = Command::new(get_ffmpeg_path());
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000);
    }

    let mut child = cmd
        .arg("-ss").arg(start_secs.to_string())
        .arg("-i").arg(file_path)
        .arg("-f").arg("s16le")
        .arg("-ar").arg("44100")
        .arg("-ac").arg("2")
        .arg("pipe:1")
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| AppError::Audio(format!("FFmpeg引擎未能启动: {}", e)))?;

    let stdout = child.stdout.take()
        .ok_or_else(|| AppError::Audio("FFmpeg stdout 不可用".into()))?;
    
    *ffmpeg_process = Some(child);
    Ok(stdout)
}

pub async fn play_local_file(
    file_path: String,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<PlayerCmd>,
    initial_loop: bool,
    target_device: Option<String>,
    app_handle: tauri::AppHandle,
    initial_volume: f32,
) -> AppResult<()> {
    let join_result = tokio::task::spawn_blocking(move || -> AppResult<()> {
        let mut current_vol = initial_volume;
        
        // 使用抽离后的函数初始化设备，忽略失败使其变为 Option
        let mut audio_output = init_audio_device(target_device.as_deref()).ok();

        if let Some((_, ref sink)) = audio_output {
            sink.set_volume(current_vol)
        }

        let mut current_time = 0f64;
        let mut is_playing = true;
        let mut is_looping = initial_loop;
        let mut last_update = Instant::now();

        let mut ffmpeg_process: Option<Child> = None;
        let mut stdout = restart_ffmpeg(0.0, &file_path, &mut ffmpeg_process).ok();

        let mut pcm_buffer = vec![0u8; 88200];
        let mut last_ui_update = Instant::now();
        
        loop {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    PlayerCmd::Pause => {
                        if is_playing {
                            if let Some((_, ref sink)) = audio_output { sink.pause(); }
                            is_playing = false;
                            current_time += last_update.elapsed().as_secs_f64();
                        }
                    }
                    PlayerCmd::SetVolume(vol) => {
                        current_vol = vol; 
                        if let Some((_, ref sink)) = audio_output { sink.set_volume(vol); }
                    }
                    PlayerCmd::Resume => {
                        if !is_playing {
                            if let Some((_, ref sink)) = audio_output { sink.play(); }
                            is_playing = true;
                            last_update = Instant::now();
                        }
                    }
                    PlayerCmd::Stop => {
                        if let Some(mut p) = ffmpeg_process.take() { let _ = p.kill(); }
                        return Ok(());
                    }
                    PlayerCmd::SetLoop(l) => is_looping = l,
                    PlayerCmd::Seek(secs) => {
                        current_time = secs as f64;
                        if let Some((_, ref sink)) = audio_output { sink.clear(); }
                        stdout = restart_ffmpeg(current_time, &file_path, &mut ffmpeg_process).ok();
                        if is_playing {
                            if let Some((_, ref sink)) = audio_output { sink.play(); }
                            last_update = Instant::now();
                        }
                    }
                    PlayerCmd::ChangeDevice(dev) => {
                        audio_output = init_audio_device(dev.as_deref()).ok();
                        if let Some((_, ref sink)) = audio_output {
                            sink.set_volume(current_vol);
                            if !is_playing { sink.pause(); }
                        }
                    }
                }
            }

            if is_playing {
                current_time += last_update.elapsed().as_secs_f64();
                last_update = Instant::now();
                if last_ui_update.elapsed() >= Duration::from_millis(500) {
                    let _ = app_handle.emit("playback-progress", current_time);
                    last_ui_update = Instant::now();
                }
            }

            let mut data_read = false;
            if is_playing {
                let mut needs_append = false;
                let mut is_empty = false;
                if let Some((_, ref sink)) = audio_output {
                    needs_append = sink.len() < 2;
                    is_empty = sink.empty();
                }

                if needs_append {
                    if let Some(out) = stdout.as_mut() {
                        match out.read(&mut pcm_buffer) {
                            Ok(0) => {
                                if is_empty {
                                    if is_looping {
                                        current_time = 0.0;
                                        last_update = Instant::now();
                                        stdout = restart_ffmpeg(0.0, &file_path, &mut ffmpeg_process).ok();
                                        if is_playing {
                                            if let Some((_, ref sink)) = audio_output { sink.play(); }
                                        }
                                    } else {
                                        break;
                                    }
                                }
                            }
                            Ok(n) => {
                                data_read = true;
                                let valid_bytes = n - (n % 2);
                                let samples: Vec<i16> = pcm_buffer[..valid_bytes]
                                    .chunks_exact(2)
                                    .map(|c| i16::from_le_bytes([c[0], c[1]]))
                                    .collect();
                                let source = rodio::buffer::SamplesBuffer::new(2, 44100, samples);
                                if let Some((_, ref sink)) = audio_output {
                                    sink.append(source);
                                }
                            }
                            Err(_) => {}
                        }
                    }
                }
            }

            if !data_read {
                std::thread::sleep(Duration::from_millis(20));
            }
        }

        if let Some(mut p) = ffmpeg_process.take() { let _ = p.kill(); }
        Ok(())
    }).await;

    match join_result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(AppError::Audio(format!("物理线程完全崩溃: {}", e))),
    }
}