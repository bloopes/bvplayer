//! 音频播放引擎模块。
//!
//! 通过 FFmpeg 子进程解码 m4s 音频为 PCM 裸流，再由 rodio 推送到声卡。
//! 支持热插拔切换输出设备、实时调节音量、Seek 跳转和单曲循环。

use std::process::{Command, Stdio, Child, ChildStdout};
use std::path::Path;
use std::io::Read;
use rodio::{OutputStream, Sink};
use std::time::{Duration, Instant};
use crate::error::{AppError, AppResult};
use tauri::Emitter;

/// 音频守护进程发送给播放引擎的控制指令。
pub enum PlayerCmd {
    Pause,
    Resume,
    Stop,
    /// 是否启用单曲循环模式。
    SetLoop(bool),
    /// 跳转到指定秒数，触发 FFmpeg 重新解码。
    Seek(u64),
    /// 实时切换音频输出设备（热插拔）。
    ChangeDevice(Option<String>),
    /// 实时调节音量，范围 0.0 ~ 1.0+。
    SetVolume(f32),
}

/// 按优先级搜索 FFmpeg 可执行文件路径。
///
/// dev 模式下 FFmpeg 可能位于项目根目录，生产模式下由 Tauri 打包到 resource 目录
/// 并通过 PATH 注入，因此需要依次尝试多个相对路径，最后回退到系统 PATH。
fn get_ffmpeg_path() -> String {
    if Path::new("./ffmpeg.exe").exists() { "./ffmpeg.exe".to_string() }
    else if Path::new("../ffmpeg.exe").exists() { "../ffmpeg.exe".to_string() }
    else if Path::new("./ffmpeg").exists() { "./ffmpeg".to_string() }
    else if Path::new("../ffmpeg").exists() { "../ffmpeg".to_string() }
    else { "ffmpeg".to_string() }
}

/// 播放本地音频文件：FFmpeg 解码 → rodio 输出 → 响应前端控制指令。
///
/// ## 架构
///
/// FFmpeg 子进程将 m4s 文件解码为 PCM S16LE 裸流（44100Hz / 立体声），
/// 通过 pipe:1 输出，Rust 侧读取后封装为 `SamplesBuffer` 推入 rodio Sink。
/// rodio 负责与操作系统音频 API 交互，支持设备枚举与热插拔。
///
/// ## 参数
/// - `file_path`: 本地缓存文件的绝对路径
/// - `rx`: 接收 `PlayerCmd` 的无界通道接收端
/// - `initial_loop`: 启动时是否开启单曲循环
/// - `target_device`: 指定输出设备名；`None` 使用系统默认
/// - `app_handle`: Tauri 句柄，用于向前端 emit 播放进度
/// - `initial_volume`: 初始音量（来自全局状态机）
///
/// ## 返回值
/// 正常播放完毕返回 `Ok(())`，错误返回 `AppError`。
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

        use rodio::cpal::traits::{HostTrait, DeviceTrait};
        let host = rodio::cpal::default_host();

        // 将 OutputStream + Sink 包装为 Option，销毁时自动释放音频设备，
        // 实现设备热插拔：切换设备时 drop 旧 pair → 创建新 pair
        let mut audio_output: Option<(OutputStream, Sink)> = None;

        // 宏封装音频设备初始化逻辑，在首次启动和热插拔时复用
        macro_rules! init_audio {
            ($dev_name_opt:expr) => {
                let mut device_opt = None;
                if let Some(dev_name) = $dev_name_opt {
                    if let Ok(mut devices) = host.output_devices() {
                        device_opt = devices.find(|d| d.name().unwrap_or_default() == *dev_name);
                    }
                }
                let device = device_opt.unwrap_or_else(|| host.default_output_device().expect("系统中未找到音频输出设备"));

                if let Ok((stream, stream_handle)) = OutputStream::try_from_device(&device) {
                    if let Ok(sink) = Sink::try_new(&stream_handle) {
                        audio_output = Some((stream, sink));
                    }
                }
            };
        }

        init_audio!(&target_device);
        if audio_output.is_none() {
            return Err(AppError::Audio("音频通道建立失败".into()));
        }

        if let Some((_,ref sink)) = audio_output {
            sink.set_volume(current_vol)
        }

        let mut current_time = 0f64;
        let mut is_playing = true;
        let mut is_looping = initial_loop;
        let mut last_update = Instant::now();

        let mut ffmpeg_process: Option<Child> = None;
        let mut stdout: Option<ChildStdout>;

        // 宏封装 FFmpeg 子进程重启逻辑，在首播 / Seek / 循环时复用
        macro_rules! restart_ffmpeg {
        ($start_secs:expr, $path:expr) => {
        if let Some(mut old) = ffmpeg_process.take() { let _ = old.kill(); }
        let mut cmd = Command::new(get_ffmpeg_path());
        // Windows 下隐藏 FFmpeg 控制台窗口（CREATE_NO_WINDOW = 0x08000000）
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }

        let mut child = cmd
            .arg("-ss").arg($start_secs.to_string())
            .arg("-i").arg($path)
            // 输出 PCM S16LE 裸流：44100Hz / 双声道，直接 pipe 到 Rust 侧处理
            .arg("-f").arg("s16le")
            .arg("-ar").arg("44100")
            .arg("-ac").arg("2")
            .arg("pipe:1")
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // 抑制 FFmpeg 日志输出
            .spawn()
            .expect("FFmpeg引擎未能启动");

        stdout = child.stdout.take();
        ffmpeg_process = Some(child);
    };
}

        restart_ffmpeg!(0.0, &file_path);
        // PCM 缓冲区：44100Hz * 2 声道 * 2 字节/样本 ≈ 0.5 秒音频
        let mut pcm_buffer = vec![0u8; 88200];
        let mut last_ui_update = Instant::now();
        loop {
            while let Ok(cmd) = rx.try_recv() {
                match cmd {
                    PlayerCmd::Pause => {
                        if is_playing {
                            if let Some((_, ref sink)) = audio_output { sink.pause(); }
                            is_playing = false;
                            // 暂停时冻结 current_time，避免 elapsed 累积导致进度跳跃
                            current_time += last_update.elapsed().as_secs_f64();
                        }
                    }
                    PlayerCmd::SetVolume(vol) => {
                        current_vol = vol; // 记住最新音量，供热插拔时恢复
                        if let Some((_, ref sink)) = audio_output {
                            sink.set_volume(vol);
                        }
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
                    PlayerCmd::SetLoop(l) => { is_looping = l; }
                    PlayerCmd::Seek(secs) => {
                        current_time = secs as f64;
                        // 清空音频缓冲区，避免 Seek 后短暂播放旧位置的数据
                        if let Some((_, ref sink)) = audio_output { sink.clear(); }
                        restart_ffmpeg!(current_time, &file_path);
                        if is_playing {
                            if let Some((_, ref sink)) = audio_output { sink.play(); }
                            last_update = Instant::now();
                        }
                    }
                    PlayerCmd::ChangeDevice(dev) => {
                        // 销毁旧设备句柄 → 创建新设备句柄 → 恢复音量和播放状态
                        audio_output = None;
                        init_audio!(&dev);
                        if let Some((_, ref sink)) = audio_output {
                            sink.set_volume(current_vol); // 换声卡后继承之前的音量
                            if !is_playing { sink.pause(); }
                        }
                    }
                }
            }

            if is_playing {
                current_time += last_update.elapsed().as_secs_f64();
                last_update = Instant::now();
                // 每 500ms 推送一次进度给前端，避免过于频繁的 IPC 调用
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
                    // 保持 sink 中至少有 2 个缓冲区待播，防止音频断流产生杂音
                    needs_append = sink.len() < 2;
                    is_empty = sink.empty();
                }

                if needs_append {
                    if let Some(out) = stdout.as_mut() {
                        match out.read(&mut pcm_buffer) {
                            Ok(0) => {
                                // FFmpeg 管道关闭（文件播完）→ 循环或结束
                                if is_empty {
                                    if is_looping {
                                        current_time = 0.0;
                                        last_update = Instant::now();
                                        restart_ffmpeg!(0.0, &file_path);
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
                                // 确保字节数对齐到 2 的倍数（i16 = 2 字节），避免立体声左右通道错乱
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
                // 无数据时短暂休眠，避免空转耗尽 CPU
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
