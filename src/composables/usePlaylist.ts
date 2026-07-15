import { ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Song } from '../types';

// 📍 新增：歌单元数据接口
export interface PlaylistMeta {
  id: string;
  name: string;
  cover: string;
  songs: Song[];
}

// 模块级单例
const allPlaylists = ref<Record<string, PlaylistMeta>>({});
const activePlaylistId = ref<string>('default');
const playlist = ref<Song[]>([]);
const currentIndex = ref<number>(-1);

let isPersistenceInitialized = false; // 防重复挂载锁 (解决问题 #7)

export function usePlaylist() {
  const saveDatabase = () => {
    localStorage.setItem('b-player-database', JSON.stringify(allPlaylists.value));
  };

  const initPersistence = () => {
    if (isPersistenceInitialized) return;

    // 解决问题 #5：统一使用 b-player-database 键名
    const saved = localStorage.getItem('b-player-database');
    if (saved) {
      try {
        allPlaylists.value = JSON.parse(saved);
        if (allPlaylists.value['default']) {
          playlist.value = allPlaylists.value['default'].songs;
        }
      } catch (e) {
        console.error('Failed to parse database:', e);
      }
    } else {
      // 首次启动初始化
      allPlaylists.value = {
        "default": {
          id: "default", name: "默认歌单",
          cover: "https://i0.hdslb.com/bfs/archive/478a05f0134440062b1aab93d62957bda52b57e9.jpg",
          songs: []
        }
      };
    }

    watch(allPlaylists, () => { saveDatabase(); }, { deep: true });
    isPersistenceInitialized = true;
  };

  // 补齐缺失的方法
  const nextSong = async () => { await invoke("play_next"); };
  const prevSong = async () => { await invoke("play_prev"); };
  
  // 解决问题 #12：优雅清空数据，不使用 reload
  const clearAllData = () => {
    localStorage.removeItem('b-player-database');
    allPlaylists.value = {
      "default": { id: "default", name: "默认歌单", cover: "", songs: [] }
    };
    playlist.value = [];
  };

  return {
    allPlaylists, activePlaylistId, playlist, currentIndex,
    initPersistence, saveDatabase, clearAllData, nextSong, prevSong
  };
}