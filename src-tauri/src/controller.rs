use crate::models::{PlaybackMode, Song}; // 📍 修复：正确引入 models.rs 中的结构
use rand::Rng;

pub struct PlaylistManager {
    songs: Vec<Song>,
    current_index: Option<usize>,
    mode: PlaybackMode, // 📍 修复：直接使用导入的 PlaybackMode，删除了本地的重复 enum
    output_device: Option<String>,
    volume: f32,
}

impl PlaylistManager {
    pub fn new(songs: Vec<Song>) -> Self {
        Self {
            songs,
            current_index: None,
            mode: PlaybackMode::ListLoop,
            output_device: None,
            volume: 1.0,
        }
    }

    pub fn set_volume(&mut self, vol: f32) { self.volume = vol; }
    pub fn get_volume(&self) -> f32 { self.volume }

    // pub fn add_song(&mut self, song: Song) {
    //     self.songs.push(song);
    //     if self.current_index.is_none() {
    //         self.current_index = Some(0);
    //     }
    // }

    pub fn get_all_songs(&self) -> &[Song] {
        &self.songs
    }

    pub fn set_mode(&mut self, mode: PlaybackMode) { self.mode = mode; }
    pub fn get_mode(&self) -> PlaybackMode { self.mode }

    pub fn set_output_device(&mut self, device: Option<String>) {
        self.output_device = device;
    }
    
    pub fn get_output_device(&self) -> Option<&str> {
        self.output_device.as_deref()
    }

    pub fn current_song(&self) -> Option<&Song> {
        if self.songs.is_empty() { return None; }
        self.current_index.and_then(|idx| self.songs.get(idx))
    }

    pub fn set_current_index(&mut self, index: usize) -> bool {
        if index < self.songs.len() {
            self.current_index = Some(index);
            true
        } else {
            false
        }
    }

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

    pub fn peek_next_song(&self) -> Option<&Song> {
        if self.songs.is_empty() { return None; }
        let next_idx = match self.mode {
            PlaybackMode::SingleLoop => self.current_index.unwrap_or(0),
            PlaybackMode::Shuffle => rand::thread_rng().gen_range(0..self.songs.len()),
            PlaybackMode::ListLoop => {
                let idx = self.current_index.unwrap_or(0);
                (idx + 1) % self.songs.len()
            }
        };
        self.songs.get(next_idx)
    }

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
            PlaybackMode::Shuffle => Some(rand::thread_rng().gen_range(0..self.songs.len()))
        };
    }

    pub fn prev_song(&mut self) {
        if self.songs.is_empty() {
            self.current_index = None;
            return;
        }
        self.current_index = match self.mode {
            PlaybackMode::SingleLoop | PlaybackMode::ListLoop => {
                let idx = self.current_index.unwrap_or(0);
                Some(if idx == 0 { self.songs.len() - 1 } else { idx - 1 })
            }
            PlaybackMode::Shuffle => Some(rand::thread_rng().gen_range(0..self.songs.len()))
        };
    }
}