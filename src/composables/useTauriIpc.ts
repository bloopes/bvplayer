import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { usePlayer } from './usePlayer';
import { usePlaylist } from './usePlaylist';

const unlistenFns: UnlistenFn[] = [];
let isListening = false; // 📍 增加竞态锁

export function useTauriIpc() {
  const { isPlaying, progress, currentSong } = usePlayer();
  const { currentIndex } = usePlaylist();

  const initIpcListeners = async () => {
    // 拦截重复注册
    if (isListening) return;
    isListening = true; 

    try {
      const unlistenProgress = await listen<number>('playback-progress', (event) => {
        progress.value = event.payload;
      });
      
      const unlistenStart = await listen<any>('playback-start', (event) => {
        currentSong.value = event.payload; 
        progress.value = 0;
        isPlaying.value = true;
      });

      unlistenFns.push(unlistenProgress, unlistenStart);
    } catch (e) {
      isListening = false; // 失败时释放锁
      console.error("Failed to init IPC listeners", e);
    }
  };

  const cleanupIpcListeners = () => {
    unlistenFns.forEach(fn => fn());
    unlistenFns.length = 0;
    isListening = false; // 📍 注销后恢复状态
  };

  return { initIpcListeners, cleanupIpcListeners };
}