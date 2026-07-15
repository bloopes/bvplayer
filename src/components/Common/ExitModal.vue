<template>
  <!-- 退出确认弹窗 -->
  <div v-if="showExitModal" class="modal-overlay" @click.self="showExitModal = false">
    <div class="modal-content exit-modal">
      <h3>关闭应用</h3>
      <p>你要将应用最小化到系统托盘，还是彻底退出？</p>
      
      <div class="modal-actions">
        <button class="btn secondary" @click="minimizeToTray">⬇️ 最小化到托盘</button>
        <button class="btn danger" @click="confirmExit">🛑 彻底退出</button>
      </div>
      <!-- 📍 新增的视觉提示 -->
      <div class="modal-hint">（点击空白处取消）</div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// 退出确认弹窗的状态
const showExitModal = ref(false);
let unlistenExit: () => void;

onMounted(async () => {
  // 监听后端发来的拦截信号
  unlistenExit = await listen('toggle-exit-modal', () => {
    showExitModal.value = true;
  });
});

onUnmounted(() => {
  if (unlistenExit) unlistenExit();
});

// 彻底退出应用
const confirmExit = async () => {
  await invoke('force_exit');
};

// 隐藏到系统托盘
const minimizeToTray = async () => {
  showExitModal.value = false;
  await invoke('hide_window');
};
</script>

<style scoped>
/* 弹窗遮罩与主体排版 (保留原样) */
.modal-overlay {
  position: fixed;
  top: 0; left: 0; right: 0; bottom: 0;
  background: rgba(17, 17, 27, 0.8);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9999;
}

.modal-content {
  background: #181825;
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  padding: 32px 24px; 
  width: 90%;
  max-width: 360px; /* 📍 核心修改：从 420px 缩小到 360px */
  text-align: center;
  box-shadow: 0 10px 30px rgba(0, 0, 0, 0.5);
}

/* 按钮容器排版 */
.modal-actions {
  display: flex;
  justify-content: center;
  gap: 12px; /* 稍微收紧按钮间距 */
  margin-top: 28px; /* 📍 微调 2：把按钮和上方文字拉开距离 */
}

/* 基础按钮样式 (对应“取消”按钮) */
.modal-actions .btn {
  /* 📍 微调 3：核心修复！让三个按钮平分容器，强制等宽 */
  flex: 1; 
  padding: 10px 0; /* 统一高度，宽度交由 flex 控制 */
  
  /* 确保图标和文字垂直居中且有间隔 */
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px; 
  
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  color: #cdd6f4;
  font-size: 0.95rem;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.modal-actions .btn:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: translateY(-1px);
}

/* 最小化按钮 (保持你的主题蓝设计) */
.modal-actions .btn.secondary {
  background: rgba(137, 180, 250, 0.15);
  border-color: rgba(137, 180, 250, 0.3);
  color: #89b4fa;
}

.modal-actions .btn.secondary:hover {
  background: #89b4fa;
  color: #11111b;
}

/* 彻底退出按钮 (保持你的危险红设计) */
.modal-actions .btn.danger {
  background: rgba(243, 139, 168, 0.15);
  border-color: rgba(243, 139, 168, 0.3);
  color: #f38ba8;
}

.modal-actions .btn.danger:hover {
  background: #f38ba8;
  color: #11111b;
}

/* 点击空白处取消的提示文字 */
.modal-hint {
  margin-top: 20px;
  font-size: 0.8rem;
  color: rgba(255, 255, 255, 0.3);
  pointer-events: none;
}
</style>