// src/composables/usePlayer.ts
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Song } from '../types';

const isPlaying = ref(false);
// 📍 修正：全局初始音量统一设为 1.0 (即 100% 满音量)，与 0~1 区间对齐
const currentVolume = ref(1.0);
const progress = ref(0);
const currentSong = ref<Song | null>(null);

export function usePlayer() {
  const togglePlay = async () => {
    try {
      if (isPlaying.value) {
        await invoke('pause_audio');
        isPlaying.value = false;
      } else {
        await invoke('resume_audio');
        isPlaying.value = true;
      }
    } catch (e) {
      console.error('Playback action failed:', e);
    }
  };

  const setVolume = async (val: number) => {
    // 📍 修正：移除 val / 100，因为组件传过来的 val 已经是 0.0 ~ 1.0 之间的浮点数
    // 保留平方曲线拟合（让低音量区间的调节更细腻，符合人耳听觉特性）
    const curvedVolume = Math.pow(val, 2); 
    
    try {
      // 📍 修正：将参数名 volume 改为后端严格要求的 vol
      await invoke('set_volume', { vol: curvedVolume });
    } catch (e) {
      console.error('Failed to set volume:', e);
    }
  };

  return {
    isPlaying, currentVolume, progress, currentSong,
    togglePlay, setVolume
  };
}