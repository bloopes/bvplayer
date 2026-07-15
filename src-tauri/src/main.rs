#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{TrayIconBuilder, TrayIconEvent},
    Manager, Emitter
};
use tokio::sync::mpsc;

// 注册新模块
mod api;
mod commands;
mod controller;
mod daemon;
mod download;
mod error;
mod player;
mod models;

use models::{AppState, UiCmd};

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
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .plugin(tauri_plugin_dialog::init())
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                println!("🔥 捕获到关闭请求，正在通知前端...");
                let _ = window.emit("toggle-exit-modal", ());
            }
            _ => {}
        })
        .setup(|app| {
            if let Ok(resource_dir) = app.path().resource_dir() {
                let path_env = std::env::var_os("PATH").unwrap_or_default();
                let mut paths = std::env::split_paths(&path_env).collect::<Vec<_>>();
                paths.push(resource_dir);
                if let Ok(new_path) = std::env::join_paths(paths) {
                    std::env::set_var("PATH", new_path);
                }
            }

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
                daemon::audio_daemon(daemon_manager, gui_rx, app_handle_for_daemon).await;
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // 路由全部转交独立模块
            commands::get_playlist,
            commands::play_prev,
            commands::play_next,
            commands::play_at_index,
            commands::sync_playlist,
            commands::set_playback_mode,
            commands::pause_audio,
            commands::resume_audio,
            commands::seek_audio,
            commands::get_audio_devices,
            commands::set_audio_device,
            commands::switch_device_realtime,
            commands::set_volume,
            commands::hide_window,
            commands::save_file,
            commands::force_exit,
            api::import_bvid,
            api::import_fav_list,
            api::import_season_list,
        ])
        .run(tauri::generate_context!())
        .expect("Tauri 主程序崩溃");
}