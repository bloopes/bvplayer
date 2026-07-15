<template>
  <div class="app-container">
    <TopSearchBar />
    <PlaylistMain />
    <BottomPlayer />
  </div>
  
  <!-- 📍 引用独立的退出确认弹窗 -->
  <ExitModal />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted } from 'vue';

import TopSearchBar from './components/Layout/TopSearchBar.vue';
import PlaylistMain from './components/Layout/PlaylistMain.vue';
import BottomPlayer from './components/Layout/BottomPlayer.vue';
// 根据你实际的路径调整引入位置
import ExitModal from './components/Common/ExitModal.vue'; 

import { usePlaylist } from './composables/usePlaylist';
import { useDevice } from './composables/useDevice';
import { useTauriIpc } from './composables/useTauriIpc';

const { initPersistence } = usePlaylist();
const { fetchDevices } = useDevice();
const { initIpcListeners, cleanupIpcListeners } = useTauriIpc();

onMounted(async () => {
  // 1. 恢复本地持久化数据
  initPersistence();
  // 2. 拉取声卡硬件列表
  await fetchDevices();
  // 3. 建立并保存 IPC 监听句柄
  await initIpcListeners();
});

onUnmounted(() => {
  // 组件卸载时释放所有监听器，物理阻断内存泄漏
  cleanupIpcListeners();
});
</script>

<style scoped>
.app-container {
  display: flex;
  flex-direction: column;
  height: 100vh;
  width: 100vw;
  overflow: hidden;
  background-color: var(--bg-color, #1e1e1e);
  color: var(--text-color, #ffffff);
}
</style>