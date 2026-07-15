<template>
  <li 
    :class="[
      'track-item', 
      { 
        'is-active': isActive, 
        'is-dragging': isDragging, 
        'delete-highlight': isDeleteMode && isHovered, 
        'is-pressed': isPressed,
      }
    ]"
    @mousedown="emit('drag-start', $event, index)" 
    @dblclick="emit('play', index)"
    @mouseenter="handleMouseEnter"
    @mouseleave="handleMouseLeave"
  >
    <img :src="song.cover_url" referrerpolicy="no-referrer" class="cover-art" />
    <div class="info">
      <div class="title">{{ song.title }}</div>
      <div class="author">{{ song.author }}</div>
    </div>
    <div class="duration">{{ formatTime(song.duration) }}</div>
  </li>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import type { Song } from '../../types';

const props = defineProps<{
  song: Song;
  index: number;
  isActive: boolean;
  isDragging: boolean;
  isDeleteMode: boolean;// 父组件传入的删除模式状态
  isPressed?: boolean; // 父组件传入的按压状态
}>();

// 用于判断悬停状态的内部变量
const isHovered = ref(false);

const handleMouseEnter = () => {
  if (props.isDeleteMode) isHovered.value = true;
};

const handleMouseLeave = () => {
  isHovered.value = false;
};

const emit = defineEmits<{
  (e: 'play', index: number): void;
  (e: 'drag-start', event: MouseEvent, index: number): void;
}>();

const formatTime = (secs: number) => {
  if (isNaN(secs)) return "0:00";
  const s = Math.floor(secs);
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
};
</script>

<style scoped>
.song-item {
  display: flex; align-items: center; gap: 18px;
  background: transparent; padding: 10px 15px; border-radius: 8px;
  border-left: 3px solid transparent; transition: all 0.2s ease;
}

.song-item:active { background: rgba(255, 255, 255, 0.05); }

.song-item.is-dragging {
  opacity: 0.5; background: #181825; transform: scale(0.98);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.3); cursor: grabbing;
}

.title, .author { user-select: text; }

.cover-art {
  width: 48px; height: 48px; border-radius: 6px;
  object-fit: cover; pointer-events: none;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2);
}

.info { flex: 1; pointer-events: none; display: flex; flex-direction: column; justify-content: center; }
.title { font-weight: 600; color: #cdd6f4; font-size: 0.95rem; margin-bottom: 4px; }
.author { font-size: 0.8rem; color: #7f849c; }
.duration {
  color: #a6adc8; font-family: ui-monospace, monospace; font-size: 0.85rem;
  background: rgba(255, 255, 255, 0.05); padding: 4px 8px; border-radius: 4px;
}
.track-item {
  display: flex; align-items: center; gap: 18px;
  background: transparent; padding: 10px 15px; border-radius: 8px;
  transition: all 0.2s ease; border-left: 3px solid transparent;
}

/* 悬停反馈 */
.track-item.delete-highlight {
  background-color: rgba(243, 139, 168, 0.15) !important;
  border-left-color: #f38ba8 !important; /* 强化红色边缘 */
  cursor: pointer;
  transform: translateX(4px);
}

/* 📍 新增：按下/长按时的瞬时物理反馈 */
.track-item.is-pressed:not(.is-dragging) {
  transform: scale(0.98); /* 微微内陷，模拟按压物理按键的感觉 */
  background-color: rgba(255, 255, 255, 0.06); /* 稍微提亮背景 */
  transition: all 0.1s ease; /* 动画时间极短，要求按下瞬间立刻响应 */
  cursor: grab;
}
</style>