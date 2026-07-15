//! 播放列表管理与播放模式控制模块。
//!
//! 提供歌曲队列的增删、播放模式切换（列表循环 / 单曲循环 / 随机）、
//! 以及音量与音频输出设备的状态管理。

use crate::api::Song;
use rand::seq::SliceRandom;

/// 播放模式枚举。
#[derive(Debug, Clone, PartialEq)]
pub enum PlaybackMode {
    /// 列表循环：播放完最后一首后回到第一首。
    ListLoop,
    /// 单曲循环：始终重复当前歌曲。
    SingleLoop,
    /// 随机播放：每次切歌时从全列表中随机选取。
    Shuffle,
}

/// 播放列表管理器，持有全部歌曲、当前播放位置、播放模式和音频设置。
pub struct PlaylistManager {
    songs: Vec<Song>,
    /// 初始为 `None`，表示尚未选中任何歌曲；用户点击播放后由前端赋值。
    current_index: Option<usize>,
    mode: PlaybackMode,
    /// 用户选择的音频输出设备；`None` 表示使用系统默认设备。
    output_device: Option<String>,
    /// 全局音量（0.0 ~ 1.0+），默认 1.0（100%）避免初始静音。
    volume: f32,
}

impl PlaylistManager {
    pub fn new(songs: Vec<Song>) -> Self {
        Self {
            songs,
            current_index: None,
            mode: PlaybackMode::ListLoop,
            output_device: None,
            volume: 1.0, // 默认 100% 音量，避免用户误以为无声
        }
    }

    pub fn set_volume(&mut self, vol: f32) { self.volume = vol; }
    pub fn get_volume(&self) -> f32 { self.volume }

    /// 追加歌曲到列表末尾。若列表之前为空，自动将新增歌曲设为当前播放项。
    pub fn add_song(&mut self, song: Song) {
        self.songs.push(song);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    pub fn get_all_songs(&self) -> &Vec<Song> { &self.songs }

    pub fn set_mode(&mut self, mode: PlaybackMode) { self.mode = mode; }
    pub fn get_mode(&self) -> PlaybackMode { self.mode.clone() }

    pub fn set_output_device(&mut self, device: Option<String>) { self.output_device = device; }
    pub fn get_output_device(&self) -> Option<String> { self.output_device.clone() }

    /// 获取当前正在播放的歌曲；列表为空或尚未选中时返回 `None`。
    pub fn current_song(&self) -> Option<&Song> {
        if self.songs.is_empty() { return None; }
        self.current_index.and_then(|idx| self.songs.get(idx))
    }

    /// 跳转到指定索引的歌曲。返回 `false` 表示索引越界。
    pub fn set_current_index(&mut self, index: usize) -> bool {
        if index < self.songs.len() {
            self.current_index = Some(index);
            true
        } else {
            false
        }
    }

    /// 用新歌曲列表替换当前列表，同时尝试保持当前播放歌曲的位置不变。
    ///
    /// 通过 BV 号匹配，避免刷新导入后播放位置被重置到第一首。
    pub fn sync_songs(&mut self, new_songs: Vec<Song>) {
        let current_bvid = self.current_song().map(|s| s.bvid.clone());
        self.songs = new_songs;
        if let Some(bvid) = current_bvid {
            self.current_index = self.songs.iter().position(|s| s.bvid == bvid);
        } else {
            if !self.songs.is_empty() {
                self.current_index = Some(0);
            } else {
                self.current_index = None;
            }
        }
    }

    /// 预览下一首歌曲（不改变当前播放位置），供前端预加载使用。
    pub fn peek_next_song(&self) -> Option<&Song> {
        if self.songs.is_empty() { return None; }
        let next_idx = match self.mode {
            PlaybackMode::SingleLoop => self.current_index.unwrap_or(0),
            PlaybackMode::Shuffle => {
                let mut rng = rand::thread_rng();
                let choices: Vec<usize> = (0..self.songs.len()).collect();
                *choices.choose(&mut rng).unwrap_or(&0)
            }
            PlaybackMode::ListLoop => {
                let idx = self.current_index.unwrap_or(0);
                (idx + 1) % self.songs.len() // 模运算实现循环回到列表首
            }
        };
        self.songs.get(next_idx)
    }

    /// 切换到下一首歌曲，行为取决于当前播放模式。
    pub fn next_song(&mut self) {
        if self.songs.is_empty() {
            self.current_index = None;
            return;
        }
        self.current_index = match self.mode {
            PlaybackMode::SingleLoop | PlaybackMode::ListLoop => {
                let idx = self.current_index.unwrap_or(self.songs.len() - 1);
                Some((idx + 1) % self.songs.len())
            }
            PlaybackMode::Shuffle => {
                let mut rng = rand::thread_rng();
                let choices: Vec<usize> = (0..self.songs.len()).collect();
                Some(*choices.choose(&mut rng).unwrap_or(&0))
            }
        };
    }

    /// 切换到上一首歌曲。列表循环模式下，第一首的前一首会回绕到最后一首。
    pub fn prev_song(&mut self) {
        if self.songs.is_empty() {
            self.current_index = None;
            return;
        }
        self.current_index = match self.mode {
            PlaybackMode::SingleLoop | PlaybackMode::ListLoop => {
                let idx = self.current_index.unwrap_or(0);
                // 索引为 0 时回绕到列表末尾，实现双向循环
                Some(if idx == 0 { self.songs.len() - 1 } else { idx - 1 })
            }
            PlaybackMode::Shuffle => {
                let mut rng = rand::thread_rng();
                let choices: Vec<usize> = (0..self.songs.len()).collect();
                Some(*choices.choose(&mut rng).unwrap_or(&0))
            }
        };
    }
}
