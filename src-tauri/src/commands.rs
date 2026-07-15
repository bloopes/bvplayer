use tauri::{AppHandle, State, Window};
use crate::error::{AppError, AppResult};
// 📍 核心修复 1：统一从 models 引入所有需要的结构体和枚举
use crate::models::{AppState, PlaybackMode, Song, UiCmd};

#[tauri::command]
pub fn save_file(path: String, contents: String) -> AppResult<()> {
    std::fs::write(path, contents)?;
    Ok(())
}

#[tauri::command]
pub async fn set_volume(vol: f32, state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::SetVolume(vol)).await
        .map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
// 📍 核心修复 2：将返回值里的 api::Song 改为干净的 Song
pub fn get_playlist(state: State<'_, AppState>) -> AppResult<Vec<Song>> {
    let manager = state.lock_manager();
    // 📍 核心修复 3：将切片 &[Song] 转换为 Vec<Song>
    Ok(manager.get_all_songs().to_vec())
}

#[tauri::command]
pub async fn play_prev(state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::Prev).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn play_next(state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::Next).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn play_at_index(index: usize, state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::PlayAt(index)).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
// 📍 核心修复 4：将入参里的 api::Song 改为干净的 Song
pub async fn sync_playlist(songs: Vec<Song>, state: State<'_, AppState>) -> AppResult<()> {
    let mut manager = state.lock_manager();
    manager.sync_songs(songs);
    Ok(())
}

#[tauri::command]
pub async fn set_playback_mode(mode: String, state: State<'_, AppState>) -> AppResult<()> {
    let pb_mode = match mode.as_str() {
        "SingleLoop" => PlaybackMode::SingleLoop,
        "Shuffle" => PlaybackMode::Shuffle,
        _ => PlaybackMode::ListLoop,
    };
    state.gui_tx.send(UiCmd::SetMode(pb_mode)).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn pause_audio(state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::Pause).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn resume_audio(state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::Resume).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn seek_audio(secs: u64, state: State<'_, AppState>) -> AppResult<()> {
    state.gui_tx.send(UiCmd::Seek(secs)).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn get_audio_devices() -> AppResult<Vec<String>> {
    use rodio::cpal::traits::{DeviceTrait, HostTrait};
    let host = rodio::cpal::default_host();
    let mut devices = Vec::new();
    if let Ok(output_devices) = host.output_devices() {
        for device in output_devices {
            if let Ok(name) = device.name() {
                if !devices.contains(&name) {
                    devices.push(name);
                }
            }
        }
    }
    Ok(devices)
}

#[tauri::command]
pub fn set_audio_device(device: String, state: State<'_, AppState>) -> AppResult<()> {
    let mut manager = state.lock_manager();
    manager.set_output_device(if device.is_empty() { None } else { Some(device) });
    Ok(())
}

#[tauri::command]
pub async fn switch_device_realtime(device: String, state: State<'_, AppState>) -> AppResult<()> {
    let dev_opt = if device.is_empty() { None } else { Some(device) };
    state.gui_tx.send(UiCmd::ChangeDevice(dev_opt)).await.map_err(|e| AppError::Audio(e.to_string()))?;
    Ok(())
}

#[tauri::command]
pub fn hide_window(window: Window) {
    let _ = window.hide();
}

#[tauri::command]
pub fn force_exit(app: AppHandle) {
    app.exit(0);
}