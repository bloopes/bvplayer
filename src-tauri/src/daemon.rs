use reqwest::Client;
use std::sync::{Arc, Mutex, MutexGuard};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use crate::controller::PlaylistManager;
use crate::models::{PlaybackMode, UiCmd, PlayerCmd, Song};
use crate::{api, download, player};

// 📍 辅助函数：安全获取锁，消除所有 .unwrap()
fn safe_lock(manager: &Arc<Mutex<PlaylistManager>>) -> MutexGuard<'_, PlaylistManager> {
    manager.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub async fn audio_daemon(
    manager_arc: Arc<Mutex<PlaylistManager>>,
    mut gui_rx: mpsc::Receiver<UiCmd>,
    app_handle: AppHandle,
) {
    let client = Client::builder()
        .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
        .build()
        .unwrap();
    let (player_done_tx, mut player_done_rx) = mpsc::channel::<u64>(1);
    let mut current_player_tx: Option<mpsc::UnboundedSender<PlayerCmd>> = None;
    let mut current_session_id: u64 = 0;

    loop {
        tokio::select! {
            Some(cmd) = gui_rx.recv() => {
                match cmd {
                    UiCmd::Next => {
                        { safe_lock(&manager_arc).next_song(); }
                        trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                    }
                    UiCmd::Prev => {
                        { safe_lock(&manager_arc).prev_song(); }
                        trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                    }
                    UiCmd::PlayAt(index) => {
                        if safe_lock(&manager_arc).set_current_index(index) {
                            trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                        }
                    }
                    UiCmd::SetVolume(vol) => {
                        safe_lock(&manager_arc).set_volume(vol);
                        if let Some(tx) = &current_player_tx {
                            let _ = tx.send(PlayerCmd::SetVolume(vol));
                        }
                    }
                    UiCmd::Pause => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(PlayerCmd::Pause); }
                    }
                    UiCmd::Resume => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(PlayerCmd::Resume); }
                    }
                    UiCmd::Seek(target_secs) => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(PlayerCmd::Seek(target_secs)); }
                    }
                    UiCmd::SetMode(mode) => {
                        safe_lock(&manager_arc).set_mode(mode.clone());
                        let is_single = mode == PlaybackMode::SingleLoop;
                        if let Some(tx) = &current_player_tx { let _ = tx.send(PlayerCmd::SetLoop(is_single)); }
                    }
                    UiCmd::ChangeDevice(dev) => {
                        if let Some(tx) = &current_player_tx { let _ = tx.send(PlayerCmd::ChangeDevice(dev)); }
                    }
                }
            }
            Some(msg_session_id) = player_done_rx.recv() => {
                if msg_session_id == current_session_id {
                    { safe_lock(&manager_arc).next_song(); }
                    trigger_play_pipeline(&manager_arc, &mut current_player_tx, &client, &player_done_tx, &mut current_session_id, app_handle.clone());
                }
            }
        }
    }
}

// 📍 辅助函数：统一处理“补全CID -> 获取直链 -> 下载缓存”的脏活累活
async fn prepare_audio_cache(song: &mut Song, client: &Client) -> Result<String, String> {
    if song.cid == 0 {
        song.cid = api::fetch_default_cid(client, &song.bvid).await.map_err(|e| e.to_string())?;
    }
    
    if download::is_cached(&song.bvid, song.cid) {
        return Ok(download::get_cache_path(&song.bvid, song.cid));
    }

    if song.audio_url.is_empty() {
        song.audio_url = api::fetch_play_url(client, &song.bvid, song.cid).await.map_err(|e| e.to_string())?;
    }

    download::stream_to_disk(client, &song.audio_url, &song.bvid, song.cid).await
}

fn trigger_play_pipeline(
    manager_arc: &Arc<Mutex<PlaylistManager>>,
    current_player_tx: &mut Option<mpsc::UnboundedSender<PlayerCmd>>,
    client: &Client,
    player_done_tx: &mpsc::Sender<u64>,
    current_session_id: &mut u64,
    app_handle: AppHandle,
) {
    let (song_opt, next_song_opt, is_single_loop, target_device, current_vol) = {
        let m = safe_lock(manager_arc);
        (
            m.current_song().cloned(),
            m.peek_next_song().cloned(),
            m.get_mode() == PlaybackMode::SingleLoop,
            m.get_output_device().map(|s| s.to_string()),
            m.get_volume(),
        )
    };

    if let Some(mut song) = song_opt {
        if let Some(tx) = current_player_tx.take() {
            let _ = tx.send(PlayerCmd::Stop);
        }
        *current_session_id += 1;
        let session_id = *current_session_id;
        
        let (player_cmd_tx, mut player_cmd_rx) = mpsc::unbounded_channel();
        *current_player_tx = Some(player_cmd_tx);
        
        let client_clone = client.clone();
        let done_tx = player_done_tx.clone();
        let app_handle_clone = app_handle.clone();

        tokio::spawn(async move {
            if let Ok(PlayerCmd::Stop) = player_cmd_rx.try_recv() { return; }

            // 📍 核心优化：复用提取后的缓存逻辑，大幅简化代码
            let local_path = match prepare_audio_cache(&mut song, &client_clone).await {
                Ok(path) => path,
                Err(e) => {
                    eprintln!("❌ [播放管线] 准备缓存失败: {}", e);
                    let _ = done_tx.send(session_id).await;
                    return;
                }
            };

            let _ = app_handle_clone.emit("playback-start", song.clone());

            if let Ok(PlayerCmd::Stop) = player_cmd_rx.try_recv() { return; }

            if let Err(e) = player::play_local_file(
                local_path, player_cmd_rx, is_single_loop, target_device, app_handle.clone(), current_vol,
            ).await {
                eprintln!("💥 [硬件中断] 音频引擎崩溃: {}", e);
            }
            let _ = done_tx.send(session_id).await;
        });

        if let Some(mut next_song) = next_song_opt {
            let client_prefetch = client.clone();
            tokio::spawn(async move {
                // 📍 核心优化：预加载复用完全相同的逻辑，并增加了错误打印
                if let Err(e) = prepare_audio_cache(&mut next_song, &client_prefetch).await {
                    eprintln!("⚠️ [预加载] 下一首缓冲失败: {}", e);
                }
            });
        }
    }
}