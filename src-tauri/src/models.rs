use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex, MutexGuard};
use tokio::sync::mpsc;

/// 📍 从 api.rs 移入：领域核心数据结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub bvid: String,
    #[serde(default)]
    pub cid: u64,
    pub title: String,
    pub author: String,
    pub cover_url: String,
    pub audio_url: String, // 暂不改为 Option 以免破坏前端现有渲染逻辑
    pub duration: u64,
}

/// 📍 从 controller.rs 移入：播放模式
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaybackMode {
    ListLoop,
    SingleLoop,
    Shuffle,
}

/// 📍 从原 state.rs / player.rs 移入：进程间通信指令
#[derive(Debug)]
pub enum UiCmd {
    Next,
    Prev,
    Pause,
    Resume,
    SetMode(PlaybackMode),
    Seek(u64),
    PlayAt(usize),
    ChangeDevice(Option<String>),
    SetVolume(f32),
}

pub enum PlayerCmd {
    Pause,
    Resume,
    Stop,
    SetLoop(bool),
    Seek(u64),
    ChangeDevice(Option<String>),
    SetVolume(f32),
}

/// 全局应用状态与锁封装
pub struct AppState {
    pub gui_tx: mpsc::Sender<UiCmd>,
    pub manager: Arc<Mutex<crate::controller::PlaylistManager>>,
}

impl AppState {
    pub fn lock_manager(&self) -> MutexGuard<'_, crate::controller::PlaylistManager> {
        self.manager.lock().unwrap_or_else(|poisoned| {
            eprintln!("⚠️ [Mutex] 检测到中毒锁，尝试恢复数据结构...");
            poisoned.into_inner()
        })
    }
}