//! 应用入口与事件循环模块。
//!
//! 负责 Tauri 窗口初始化、系统托盘、GUI 命令路由、音频守护进程
//! 以及播放管道的编排。前端通过 `#[tauri::command]` 发送指令，
//! 经由 `mpsc` 通道传递给音频守护进程统一调度。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use reqwest::Client;
use std::sync::{Arc, Mutex};
use tauri::State;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::mpsc;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
};
use api::{import_fav_list, import_season_list, import_bvid};
mod api;
mod controller;
mod download;
mod error;
mod player;

/// 前端 GUI 发送给音频守护进程的控制指令。
///
/// 使用 `mpsc` 通道解耦前端事件与音频引擎，避免阻塞 Tauri 主线程。
#[derive(Debug)]
pub enum UiCmd {
    Next,
    Prev,
    Pause,
    Resume,
    SetMode(controller::PlaybackMode),
    /// 跳转到指定秒数。
    Seek(u64),
    /// 跳转到播放列表中指定索引的歌曲。
    PlayAt(usize),
    /// 切换音频输出设备；`None` 表示回退到系统默认。
    ChangeDevice(Option<String>),
    /// 设置全局音量，范围 0.0 ~ 1.0。
    SetVolume(f32),
}

/// 应用全局共享状态，通过 Tauri 的 `manage` 注入。
pub struct AppState {
    /// GUI → 守护进程的指令发送端。
    pub gui_tx: mpsc::Sender<UiCmd>,
    /// 播放列表管理器，被 GUI 和守护进程共享访问。
    pub manager: Arc<Mutex<controller::PlaylistManager>>,
}

/// 将文本内容写入本地文件。
///
/// 独立于音频管道的通用文件保存指令，前端用于导出播放列表等场景。
#[tauri::command]
fn save_file(path: String, contents: String) -> Result<(), String> {
    std::fs::write(path, contents).map_err(|e| e.to_string())
}

/// 前端调节音量 → 发送 `SetVolume` 到守护进程。
#[tauri::command]
async fn set_volume(vol: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::SetVolume(vol))
        .await
        .map_err(|e| e.to_string())
}

/// 获取当前播放列表快照，供前端渲染。
#[tauri::command]
fn get_playlist(state: State<'_, AppState>) -> Result<Vec<api::Song>, String> {
    let manager = state.manager.lock().unwrap();
    Ok(manager.get_all_songs().clone())
}

#[tauri::command]
async fn play_prev(state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::Prev)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn play_next(state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::Next)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn play_at_index(index: usize, state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::PlayAt(index))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 同步播放列表（刷新导入后调用），同时保留当前播放位置。
#[tauri::command]
async fn sync_playlist(songs: Vec<api::Song>, state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.sync_songs(songs);
    Ok(())
}

/// 切换播放模式（列表循环 / 单曲循环 / 随机）。
///
/// 前端以字符串形式传入，后端解析后同步更新状态机
/// 并通知当前播放器调整循环行为。
#[tauri::command]
async fn set_playback_mode(mode: String, state: State<'_, AppState>) -> Result<(), String> {
    let pb_mode = match mode.as_str() {
        "SingleLoop" => controller::PlaybackMode::SingleLoop,
        "Shuffle" => controller::PlaybackMode::Shuffle,
        _ => controller::PlaybackMode::ListLoop,
    };
    state
        .gui_tx
        .send(UiCmd::SetMode(pb_mode))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn pause_audio(state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::Pause)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_audio(state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::Resume)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn seek_audio(secs: u64, state: State<'_, AppState>) -> Result<(), String> {
    state
        .gui_tx
        .send(UiCmd::Seek(secs))
        .await
        .map_err(|e| e.to_string())
}

/// 枚举系统所有可用的音频输出设备名称。
///
/// 通过 cpal 底层获取，去重后返回给前端供用户选择。
#[tauri::command]
fn get_audio_devices() -> Result<Vec<String>, String> {
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

/// 持久化保存用户选择的音频设备（不触发实时切换）。
#[tauri::command]
fn set_audio_device(device: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut manager = state.manager.lock().unwrap();
    manager.set_output_device(if device.is_empty() {
        None
    } else {
        Some(device)
    });
    Ok(())
}

/// 实时切换音频输出设备，通知当前播放器立即应用。
#[tauri::command]
async fn switch_device_realtime(device: String, state: State<'_, AppState>) -> Result<(), String> {
    let dev_opt = if device.is_empty() {
        None
    } else {
        Some(device)
    };
    state
        .gui_tx
        .send(UiCmd::ChangeDevice(dev_opt))
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 隐藏窗口到系统托盘（而非关闭）。
#[tauri::command]
fn hide_window(window: tauri::Window) {
    let _ = window.hide();
}

/// 强制退出整个应用，绕过 Tauri 的窗口关闭拦截。
#[tauri::command]
fn force_exit(app: tauri::AppHandle) {
    app.exit(0);
}

/// 音频守护进程：单线程事件循环，统一调度所有播放指令。
///
/// 通过 `tokio::select!` 同时监听两类事件：
/// - `gui_rx`: 前端发来的用户操作（切歌、暂停、音量等）
/// - `player_done_rx`: 当前歌曲播放完毕后自动切下一首
///
/// 使用 `session_id` 机制防止旧播放任务结束信号误触发切歌。
async fn audio_daemon(
    manager_arc: Arc<Mutex<controller::PlaylistManager>>,
    mut gui_rx: mpsc::Receiver<UiCmd>,
    app_handle: AppHandle,
) {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap();
    let (player_done_tx, mut player_done_rx) = mpsc::channel::<u64>(1);
    let mut current_player_tx: Option<tokio::sync::mpsc::UnboundedSender<player::PlayerCmd>> = None;
    let mut current_session_id: u64 = 0;
    loop {
        tokio::select! {
            Some(cmd) = gui_rx.recv() => {
                match cmd {
                    UiCmd::Next => {
                        { manager_arc.lock().unwrap().next_song(); }
                        trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                    }
                    UiCmd::Prev => {
                        { manager_arc.lock().unwrap().prev_song(); }
                        trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                    }
                    UiCmd::PlayAt(index) => {
                        let success = { manager_arc.lock().unwrap().set_current_index(index) };
                        if success {
                            trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                        }
                    }
                    UiCmd::SetVolume(vol) => {
                        // 同时更新状态机（持久化）和当前播放器（实时生效）
                        manager_arc.lock().unwrap().set_volume(vol);
                        if let Some(tx) = &current_player_tx {
                            let _ = tx.send(player::PlayerCmd::SetVolume(vol));
                        }
                    }
                    UiCmd::Pause => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(player::PlayerCmd::Pause); }
                    }
                    UiCmd::Resume => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(player::PlayerCmd::Resume); }
                    }
                    UiCmd::Seek(target_secs) => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(player::PlayerCmd::Seek(target_secs)); }
                    }
                    UiCmd::SetMode(mode) => {
                        manager_arc.lock().unwrap().set_mode(mode.clone());
                        let is_single = mode == controller::PlaybackMode::SingleLoop;
                        if let Some(tx) = &current_player_tx { let _ = tx.send(player::PlayerCmd::SetLoop(is_single)); }
                    }
                    UiCmd::ChangeDevice(dev) => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(player::PlayerCmd::ChangeDevice(dev)); }
                    }
                }
            }
            Some(msg_session_id) = player_done_rx.recv() => {
                // 仅当 session_id 匹配时才自动切歌，避免旧任务触发误切
                if msg_session_id == current_session_id {
                    { manager_arc.lock().unwrap().next_song(); }
                    trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                }
            }
        }
    }
}

/// 播放管道核心：CID 补全 → 音频流解析 → 缓存下载 → 启动播放器 → 预加载下一首。
///
/// 每一步失败都会提前中止并通过 `player_done_tx` 触发自动切歌。
/// 通过 `session_id` 自增机制，旧管道发送的 `Stop` 信号只影响旧播放器。
fn trigger_play_pipeline(
    manager_arc: &Arc<Mutex<controller::PlaylistManager>>,
    current_player_tx: &mut Option<tokio::sync::mpsc::UnboundedSender<player::PlayerCmd>>,
    client: &reqwest::Client,
    player_done_tx: &mpsc::Sender<u64>,
    current_session_id: &mut u64,
    app_handle: AppHandle,
) {
    // 在锁内一次性提取所有需要的状态，避免多次加锁
    let (song_opt, next_song_opt, is_single_loop, target_device, current_vol) = {
        let m = manager_arc.lock().unwrap();
        (
            m.current_song().cloned(),
            m.peek_next_song().cloned(),
            m.get_mode() == controller::PlaybackMode::SingleLoop,
            m.get_output_device(),
            m.get_volume(),
        )
    };

    if let Some(mut song) = song_opt {
        // 停止当前正在播放的旧任务，释放音频设备
        if let Some(tx) = current_player_tx.take() {
            let _ = tx.send(player::PlayerCmd::Stop);
        }
        // 自增 session_id，使旧异步任务发送的 done 信号自动失效
        *current_session_id += 1;
        let session_id = *current_session_id;
        let (player_cmd_tx, mut player_cmd_rx) = tokio::sync::mpsc::unbounded_channel();
        *current_player_tx = Some(player_cmd_tx);
        let client_clone = client.clone();
        let done_tx = player_done_tx.clone();
        let bv_id = song.bvid.clone();
        let app_handle_clone = app_handle.clone();

        tokio::spawn(async move {
            if let Ok(player::PlayerCmd::Stop) = player_cmd_rx.try_recv() { return; }

            // 收藏夹 / 合集导入的数据 cid 为 0，需通过 view API 补全
            let mut actual_cid = song.cid;
            if actual_cid == 0 {
                if let Ok(real_cid) = api::fetch_default_cid(&client_clone, &song.bvid).await {
                    actual_cid = real_cid;
                    song.cid = real_cid;
                } else {
                    let _ = done_tx.send(session_id).await;
                    return;
                }
            }

            // 未缓存时调用 playurl API 获取带签名的音频直链（防盗链有时效限制）
            if !download::is_cached(&song.bvid, actual_cid) {
                if let Ok(url) = api::fetch_play_url(&client_clone, &song.bvid, actual_cid).await {
                    song.audio_url = url;
                } else {
                    let _ = done_tx.send(session_id).await;
                    return;
                }
            }

            let _ = app_handle_clone.emit("playback-start", song.clone());

            // 优先使用本地缓存，未命中则触发下载
            let local_path = if download::is_cached(&bv_id, actual_cid) {
                download::get_cache_path(&bv_id, actual_cid)
            } else {
                match download::stream_to_disk(&client_clone, &song.audio_url, &bv_id, actual_cid).await {
                    Ok(path) => path,
                    Err(_) => {
                        let _ = done_tx.send(session_id).await;
                        return;
                    }
                }
            };

            if let Ok(player::PlayerCmd::Stop) = player_cmd_rx.try_recv() { return; }

            if let Err(e) = player::play_local_file(
                local_path,
                player_cmd_rx,
                is_single_loop,
                target_device.clone(),
                app_handle.clone(),
                current_vol,
            ).await {
                eprintln!("💥 [硬件中断] 音频引擎崩溃: {}", e);
            }
            let _ = done_tx.send(session_id).await;
        });

        // 预加载下一首：在后台补全 cid → 获取 URL → 下载到缓存 → 实现切歌秒播
        if let Some(next_song) = next_song_opt {
            let next_bvid = next_song.bvid.clone();
            let mut next_cid = next_song.cid;
            let client_prefetch = client.clone();

            tokio::spawn(async move {
                if next_cid == 0 {
                    if let Ok(real_cid) = api::fetch_default_cid(&client_prefetch, &next_bvid).await {
                        next_cid = real_cid;
                    } else {
                        return;
                    }
                }

                if !download::is_cached(&next_bvid, next_cid) {
                    let mut dl_url = next_song.audio_url.clone();
                    if dl_url.is_empty() {
                        if let Ok(url) = api::fetch_play_url(&client_prefetch, &next_bvid, next_cid).await {
                            dl_url = url;
                        } else { return; }
                    }
                    let _ = download::stream_to_disk(&client_prefetch, &dl_url, &next_bvid, next_cid).await;
                }
            });
        }
    }
}

#[tokio::main]
async fn main() {
    let manager = controller::PlaylistManager::new(vec![]);
    let shared_manager = Arc::new(Mutex::new(manager));
    let daemon_manager = shared_manager.clone();

    let (gui_tx, gui_rx) = mpsc::channel::<UiCmd>(100);
    let app_state = AppState {
        gui_tx,
        manager: shared_manager,
    };

    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_dialog::init())
        // 拦截窗口关闭事件，改为弹出确认对话框而非直接退出
        .on_window_event(|window, event| match event {
        tauri::WindowEvent::CloseRequested { api, .. } => {
        api.prevent_close();
        println!("🔥 捕获到关闭请求，正在通知前端...");
        let _ = window.emit("toggle-exit-modal", ());
        }
            _ => {}
        })
        .setup(|app| {
            // 将打包附带的 FFmpeg 二进制所在目录追加到 PATH，
            // 确保 rodio 等音频库能在运行时找到 FFmpeg 解码器
            if let Ok(resource_dir) = app.path().resource_dir() {
                let path_env = std::env::var_os("PATH").unwrap_or_default();
                let mut paths = std::env::split_paths(&path_env).collect::<Vec<_>>();
                paths.push(resource_dir);
                if let Ok(new_path) = std::env::join_paths(paths) {
                    std::env::set_var("PATH", new_path);
                }
            }

            // 构建系统托盘菜单
            let quit_i = MenuItem::with_id(app, "quit", "彻底退出", true, None::<&str>)?;
            let show_i = MenuItem::with_id(app, "show", "显示主界面", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_i, &quit_i])?;

            let icon = app.default_window_icon().unwrap().clone();

            let _tray = TrayIconBuilder::new()
                .icon(icon)
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "quit" => app.exit(0),
                    "show" => {
                        if let Some(window) = app.get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::DoubleClick { .. } = event {
                        if let Some(window) = tray.app_handle().get_webview_window("main") {
                            window.show().unwrap();
                            window.set_focus().unwrap();
                        }
                    }
                })
                .build(app)?;
            let app_handle_for_daemon = app.handle().clone();
            tokio::spawn(async move {
                audio_daemon(daemon_manager, gui_rx, app_handle_for_daemon).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_playlist,
            play_prev,
            play_next,
            play_at_index,
            sync_playlist,
            api::import_bvid,
            set_playback_mode,
            pause_audio,
            resume_audio,
            seek_audio,
            get_audio_devices,
            set_audio_device,
            switch_device_realtime,
            set_volume,
            hide_window,
            save_file,
            import_fav_list,
            import_season_list,
            import_bvid,
            force_exit,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 主程序崩溃");
}
