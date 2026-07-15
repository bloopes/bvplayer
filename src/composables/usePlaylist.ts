import { ref, watch, onMounted} from 'vue';
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

    // 1. 读取数据库总清单
    const saved = localStorage.getItem('b-player-database');
    if (saved) {
      try {
        allPlaylists.value = JSON.parse(saved);
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

    // 📍 核心修复 1：读取上次退出时停留的歌单 ID (断点记忆)
    const savedActiveId = localStorage.getItem('b-player-active-id');
    if (savedActiveId && allPlaylists.value[savedActiveId]) {
      activePlaylistId.value = savedActiveId;
    } else {
      activePlaylistId.value = 'default';
    }

    // 📍 核心修复 2：确保启动时，playlist 引用精准指向当前活动的歌单
    if (allPlaylists.value[activePlaylistId.value]) {
      playlist.value = allPlaylists.value[activePlaylistId.value].songs;
    }

    // 深度监听总数据，一旦任何引用发生变动自动落盘
    watch(allPlaylists, () => { saveDatabase(); }, { deep: true });

    // 📍 核心修复 3：当用户切换歌单时，同步更新本地记忆，并重置 playlist 的数据指针
    watch(activePlaylistId, (newId) => {
      if (newId && allPlaylists.value[newId]) {
        // 将指针指向新歌单的数组，确保后续的前端操作能准确映射进 allPlaylists
        playlist.value = allPlaylists.value[newId].songs;
        // 记住当前的浏览状态
        localStorage.setItem('b-player-active-id', newId);
      }
    });

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
// 1. 在应用加载时，尝试恢复上次的 UI 状态
onMounted(() => {
  const savedActiveId = localStorage.getItem('last_active_playlist_id');
  
  // 如果找到了上次的歌单ID，并且该歌单没被删除，就恢复它
  if (savedActiveId && allPlaylists.value[savedActiveId]) {
    activePlaylistId.value = savedActiveId;
    // 顺便把当前 playlist 数组也自动填充为该歌单的歌曲
    playlist.value = [...allPlaylists.value[savedActiveId].songs];
  }
});

// 2. 监听状态变化，一旦切换歌单，立刻静默记录到本地
watch(activePlaylistId, (newId) => {
  if (newId) {
    localStorage.setItem('last_active_playlist_id', newId);
  }
});