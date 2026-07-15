<script setup lang="ts">
import { ref } from 'vue';
import { usePlayer } from '../../composables/usePlayer';

const { currentVolume, setVolume } = usePlayer();
const isMuted = ref(Number(currentVolume.value) === 0);
// 修正：初始记忆音量设为 0.5 (即 50%)，确保不超出 0.0~1.0 的量程
const previousVolume = ref<number>(0.5);

// 统一命名：处理滑块拖动事件
function handleVolumeChange(event: Event) {
  const target = event.target as HTMLInputElement;
  const val = parseFloat(target.value);
  
  currentVolume.value = val;
  isMuted.value = val === 0;
  
  // 将浮点数发给底层
  setVolume(val);
}

function toggleMute() {
  if (isMuted.value) {
    // 恢复之前的音量，如果之前是 0，则默认给到 0.5
    currentVolume.value = previousVolume.value > 0 ? previousVolume.value : 0.5;
  } else {
    // 记录当前音量，并设为 0
    previousVolume.value = Number(currentVolume.value);
    currentVolume.value = 0;
  }
  
  isMuted.value = Number(currentVolume.value) === 0;
  setVolume(Number(currentVolume.value));
}
</script>

<template>
  <div class="bili-volume-wrap">
    <div class="volume-icon" @click="toggleMute" title="静音 (m)">
      <!-- 修正：统一使用 currentVolume 渲染视图 -->
      {{ isMuted ? '🔇' : (currentVolume > 0.5 ? '🔊' : (currentVolume > 0 ? '🔉' : '🔇')) }}
    </div>
    <div class="volume-slider-drawer">
      <!-- 修正：类名改为 bili-slider，并绑定 currentVolume -->
      <input 
        type="range" 
        class="bili-slider" 
        min="0" 
        max="1" 
        step="0.01" 
        :value="currentVolume" 
        @input="handleVolumeChange"
        title="音量调节"
        :style="{ '--vol-percent': (currentVolume * 100) + '%' }"
      />
    </div>
  </div>
</template>

<style scoped>
/* 此处保留你原有的全部样式，无需修改，因为类名已经对齐 */
.bili-volume-wrap {
  display: flex;
  align-items: center;
  height: 100%;
  padding: 0 10px;
  margin: 0 -5px;
}

.volume-icon {
  font-size: 1.2rem;
  cursor: pointer;
  color: #c9d1d9;
  transition: color 0.2s;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 30px;
}

.volume-icon:hover { color: #89b4fa; }

.volume-slider-drawer {
  width: 0;
  opacity: 0;
  overflow: hidden;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              margin 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  transition-delay: 0.25s;
  display: flex;
  align-items: center;
}

.bili-volume-wrap:hover .volume-slider-drawer {
  width: 80px;
  opacity: 1;
  margin-left: 8px;
  transition-delay: 0s;
}

.bili-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 4px;
  border-radius: 2px;
  outline: none;
  cursor: pointer;
  border: none !important;
  padding: 0 !important;
  /* 📍 核心：利用 CSS 变量动态渲染左侧填充色（蓝）和右侧轨道色（灰） */
  background: linear-gradient(
    to right, 
    #89b4fa 0%, 
    #89b4fa var(--vol-percent, 50%), 
    rgba(255, 255, 255, 0.1) var(--vol-percent, 50%), 
    rgba(255, 255, 255, 0.1) 100%
  );
  transition: background 0.2s ease;
}

/* 📍 鼠标悬停轨道时，未填充的部分微微提亮，增加互动感 */
.bili-slider:hover {
  background: linear-gradient(
    to right, 
    #89b4fa 0%, 
    #89b4fa var(--vol-percent, 50%), 
    rgba(255, 255, 255, 0.2) var(--vol-percent, 50%), 
    rgba(255, 255, 255, 0.2) 100%
  );
}

.bili-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #89b4fa;
  box-shadow: 0 0 8px rgba(137, 180, 250, 0.6);
  transition: transform 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275), box-shadow 0.2s;
}

.bili-slider::-webkit-slider-thumb:hover {
  transform: scale(1.35);
  box-shadow: 0 0 12px rgba(137, 180, 250, 0.9);
}
</style>