<script setup lang="ts">
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import VolumeSlider from '../Common/VolumeSlider.vue';
import { usePlayer } from '../../composables/usePlayer';
import { useDevice } from '../../composables/useDevice';
import { usePlaylist } from '../../composables/usePlaylist';

const { isPlaying, progress, togglePlay, currentSong } = usePlayer();
const { deviceList, currentDevice, switchDevice } = useDevice();
const { nextSong, prevSong } = usePlaylist(); // 假设已在 usePlaylist 中实现

const progressBarRef = ref<HTMLElement | null>(null);
const isDragging = ref(false);
const seekLock = ref(false);

const currentMode = ref("ListLoop");
const modeIcons: Record<string, string> = { "ListLoop": "🔁", "SingleLoop": "🔂", "Shuffle": "🔀" };
const modeTitles: Record<string, string> = { "ListLoop": "列表循环", "SingleLoop": "单曲循环", "Shuffle": "随机播放" };

// 时间格式化
const formatTime = (secs: number) => {
  if (isNaN(secs)) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
};

// 📍 播放模式切换
async function toggleMode() {
  const modes = ["ListLoop", "SingleLoop", "Shuffle"];
  currentMode.value = modes[(modes.indexOf(currentMode.value) + 1) % modes.length];
  await invoke("set_playback_mode", { mode: currentMode.value });
}

// 📍 进度条拖拽核心计算
function calculateProgressFromX(clientX: number) {
  if (!progressBarRef.value || !currentSong.value) return;
  const rect = progressBarRef.value.getBoundingClientRect();
  let offsetX = Math.max(0, Math.min(clientX - rect.left, rect.width));
  progress.value = (offsetX / rect.width) * currentSong.value.duration;
}

function onMouseDown(e: MouseEvent) {
  if (!currentSong.value) return;
  isDragging.value = true;
  calculateProgressFromX(e.clientX);

  const handleMouseMove = (moveEvent: MouseEvent) => {
    calculateProgressFromX(moveEvent.clientX);
  };

  const handleMouseUp = async (upEvent: MouseEvent) => {
    calculateProgressFromX(upEvent.clientX);
    seekLock.value = true;
    isDragging.value = false;
    setTimeout(() => { seekLock.value = false; }, 800);
    
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', handleMouseUp);
    
    try {
      await invoke('seek_audio', { secs: Math.floor(progress.value) });
    } catch (err) { console.error("Seek error:", err); }
  };

  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', handleMouseUp);
}
</script>

<template>
  <footer class="player-console">
    <!-- 进度条 -->
    <div class="progress-bar-bg" v-if="currentSong" ref="progressBarRef" @mousedown="onMouseDown">
      <div class="progress-fill" :class="{ 'is-dragging': isDragging }"
        :style="{ width: (progress / currentSong.duration) * 100 + '%' }"></div>
    </div>
    
    <div class="console-inner">
      <!-- 左侧歌曲信息 -->
      <div class="now-playing" v-if="currentSong">
        <img :src="currentSong.cover_url" referrerpolicy="no-referrer" class="np-cover" />
        <div class="np-info">
          <div class="np-title">{{ currentSong.title }}</div>
          <div class="np-time">{{ formatTime(progress) }} / {{ formatTime(currentSong.duration) }}</div>
        </div>
      </div>
      <div class="now-playing empty" v-else>⚡ 音频待命...</div>

      <!-- 右侧控制区 -->
      <div class="control-hub">
        <!-- 设备选择器 -->
        <select v-model="currentDevice" @change="switchDevice(currentDevice)" class="device-selector" title="输出硬件">
          <option value="">💻 系统默认输出</option>
          <option v-for="dev in deviceList" :key="dev" :value="dev">{{ dev }}</option>
        </select>

        <div class="control-divider"></div>

        <!-- 音量抽屉组件 -->
        <VolumeSlider />

        <div class="control-divider"></div>

        <button @click="toggleMode" class="btn-icon mode-btn" :title="modeTitles[currentMode]">
          {{ modeIcons[currentMode] }}
        </button>
        <button @click="prevSong" class="btn-icon" title="上一首">⏮️</button>
        <button @click="togglePlay" class="btn-play" :disabled="!currentSong" :title="isPlaying ? '暂停' : '播放'">
          {{ isPlaying ? '⏸️' : '▶️' }}
        </button>
        <button @click="nextSong" class="btn-icon" title="下一首">⏭️</button>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.player-console {
  background: rgba(24, 24, 37, 0.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  position: relative;
  z-index: 10;
  flex-shrink: 0;
}

.progress-bar-bg {
  width: 100%; height: 8px;
  background: rgba(255, 255, 255, 0.08);
  position: absolute; top: -8px;
  cursor: pointer; transition: background 0.2s;
}
.progress-bar-bg:hover { background: rgba(255, 255, 255, 0.15); }

.progress-fill {
  height: 100%; background: #a6e3a1;
  transition: width 0.5s linear;
  box-shadow: 0 0 12px rgba(166, 227, 161, 0.7);
  border-radius: 0 4px 4px 0;
}
.progress-fill.is-dragging { transition: none !important; }

.console-inner {
  padding: 15px 30px; display: flex;
  justify-content: space-between; align-items: center;
  height: 85px;
}

.now-playing { display: flex; align-items: center; gap: 18px; width: 35%; }
.now-playing.empty { color: #585b70; font-family: monospace; font-size: 0.9rem; }
.np-cover { width: 56px; height: 56px; border-radius: 8px; object-fit: cover; box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3); }
.np-title { font-weight: 700; color: #cdd6f4; font-size: 1.05rem; margin-bottom: 4px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; max-width: 250px; }
.np-time { color: #7f849c; font-family: ui-monospace, monospace; font-size: 0.85rem; }

.control-hub { display: flex; gap: 15px; align-items: center; }
.btn-icon { background: transparent; border: none; color: #a6adc8; padding: 10px; border-radius: 8px; font-size: 1.1rem; cursor: pointer; }
.btn-icon:hover { background: rgba(255, 255, 255, 0.05); color: #cdd6f4; }

.device-selector {
  background: rgba(17, 17, 27, 0.6); border: 1px solid rgba(255, 255, 255, 0.1);
  color: #a6adc8; padding: 6px 14px; border-radius: 20px; font-size: 0.85rem;
  outline: none; max-width: 140px; cursor: pointer; transition: all 0.2s; appearance: none; text-align: center;
}
.device-selector:hover { background: rgba(255, 255, 255, 0.05); color: #cdd6f4; border-color: rgba(255, 255, 255, 0.2); }

.device-selector option {
  background-color: #181825; /* 强制设为深色背景（与主界面呼应） */
  color: #cdd6f4; /* 强制文字变为明亮的浅色 */
  font-size: 0.9rem; /* 稍微放大一点点字号，提升可读性 */
  padding: 8px; 
}
.control-divider { width: 1px; height: 18px; background: rgba(255, 255, 255, 0.1); margin: 0 4px; }

.btn-play {
  background: #cdd6f4; border: none; color: #11111b; width: 42px; height: 42px;
  border-radius: 50%; display: flex; align-items: center; justify-content: center;
  font-size: 1.2rem; cursor: pointer; box-shadow: 0 4px 12px rgba(205, 214, 244, 0.2);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1); margin: 0 6px;
}
.btn-play:hover:not(:disabled) { background: #ffffff; transform: scale(1.08); box-shadow: 0 6px 18px rgba(205, 214, 244, 0.4); }
.btn-play:active:not(:disabled) { transform: scale(0.95); }
.btn-play:disabled { background: #313244; color: #585b70; cursor: not-allowed; box-shadow: none; }
</style>