<script setup lang="ts">
import { open } from '@tauri-apps/plugin-shell';
import { ref, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { save } from '@tauri-apps/plugin-dialog';
import { usePlaylist } from '../../composables/usePlaylist';
import { useToast } from '../../composables/useToast';

const { allPlaylists, saveDatabase, restoreDatabase } = usePlaylist();
const { showToast } = useToast();

// 📍 1. 弹窗状态管理
const showSettingsModal = ref(false);
const showReadme = ref(false);
const importFileRef = ref<HTMLInputElement | null>(null);

// 📍 2. 全局事件监听：接听来自其他组件的“打开设置”呼叫
const handleOpenSettings = () => { 
  showSettingsModal.value = true; 
};

onMounted(() => { 
  window.addEventListener('open-settings', handleOpenSettings as EventListener); 
});

onUnmounted(() => { 
  window.removeEventListener('open-settings', handleOpenSettings as EventListener); 
});

// 📍 3. 关闭弹窗
function closeSettings() {
  showSettingsModal.value = false;
}

// 📍 新增打开外部链接的函数
async function openGithub() {
  try {
    // 替换为你的真实 GitHub 仓库地址
    await open('https://github.com/bloopes/bvplayer'); 
  } catch (error) {
    console.error("无法打开外部链接:", error);
  }
}

// 📍 4. 导出备份 (原样保留)
async function exportDatabase() {
  try {
    const dataStr = JSON.stringify(allPlaylists.value, null, 2);
    const filePath = await save({
      title: "导出播放器备份",
      defaultPath: `bvplayer_backup_${new Date().toISOString().split('T')[0]}.json`,
      filters: [{ name: 'JSON 数据', extensions: ['json'] }]
    });

    if (filePath) {
      await invoke('save_file', { path: filePath, contents: dataStr });
      showToast("✅ 数据备份已成功导出！", "success");
    }
  } catch (error) {
    showToast("❌ 导出失败，请检查控制台。", "error");
  }
}

// 📍 5. 导入恢复 (原样保留)
function triggerImport() { importFileRef.value?.click(); }

function onImportFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) return;
  
  const reader = new FileReader();
  reader.onload = (e) => {
    try {
      // 1. 获取原始的 JSON 字符串
      let rawText = e.target?.result as string;

      // 📍 数据清洗核心：全局扫描并替换掉带空格的非法键名
      // 这样即便你导入的是以前带有 "cover url" 的错误备份文件，也能被热修复
      rawText = rawText.replace(/"cover url"\s*:/g, '"cover_url":');
      rawText = rawText.replace(/"audio url"\s*:/g, '"audio_url":');

      // 2. 解析清洗后的干净数据
      const parsedData = JSON.parse(rawText);
      if (!parsedData || !parsedData['default']) throw new Error("无效格式");
      
      // 📍 指针解绑核心：调用我们新写的安全恢复函数，而不是直接赋值！
      if (restoreDatabase) {
        restoreDatabase(parsedData);
      } else {
        // Fallback 防御，万一忘了导出
        allPlaylists.value = parsedData;
        saveDatabase();
      }

      showToast("✅ 数据恢复成功！", "success");
      showSettingsModal.value = false;
    } catch (error) {
      console.error("Backup Parse Error:", error);
      showToast("❌ 导入失败：文件格式错误。", "error");
    } finally {
      input.value = "";
    }
  };
  reader.readAsText(input.files[0]);
}

// 📍 6. 恢复出厂设置 (原样保留)
function executeClearAllData() {
  if (confirm("⚠️ 确定要清空所有数据吗？此操作将永久丢失所有歌单，且不可恢复！")) {
    localStorage.removeItem('b-player-database');
    window.location.reload(); 
  }
}
</script>

<template>
  <!-- 隐藏的 input -->
  <input type="file" ref="importFileRef" accept="application/json" style="display: none;" @change="onImportFileSelected" />

  <!-- 📍 核心修复：使用 Teleport 将弹窗强行传送到 body 根节点，并绑定 v-if -->
  <Teleport to="body">
    
    <!-- 设置面板主体 -->
    <div v-if="showSettingsModal" class="settings-modal-overlay" @click.self="closeSettings">
      <div class="settings-modal-content">
        <header class="settings-header">
          <h2>⚙️ 系统设置</h2>
          <button class="close-btn" @click="closeSettings">×</button>
        </header>

        <div class="settings-body">
          <!-- 模块 1：数据管理 -->
          <section class="setting-card">
            <div class="setting-info">
              <h3>💾 数据管理</h3>
              <p>将你的本地歌单导出为 JSON 文件，或从备份文件中恢复。</p>
            </div>
            <div class="setting-actions">
              <button class="btn secondary" @click="exportDatabase">
                <span>📤</span> 导出备份
              </button>
              <button class="btn secondary" @click="triggerImport">
                <span>📥</span> 导入恢复
              </button>
            </div>
          </section>

          <!-- 模块 2：关于应用 -->
          <section class="setting-card">
            <div class="setting-info">
              <h3>ℹ️ 关于应用</h3>
              <p>查看版本信息、开发说明与使用指南。</p>
            </div>
            <div class="setting-actions">
              <button class="btn secondary" @click="showReadme = true">
                <span>📖</span> 查看 README
              </button>
            </div>
          </section>

          <!-- 模块 3：危险区域 -->
          <section class="setting-card danger-zone">
            <div class="setting-info">
              <h3>⚠️ 危险区域</h3>
              <p>清空所有本地数据，恢复至初始状态。此操作不可逆。</p>
            </div>
            <div class="setting-actions">
              <button class="btn danger" @click="executeClearAllData">
                <span>🗑️</span> 恢复出厂设置
              </button>
            </div>
          </section>
        </div>
      </div>
    </div>

    <!-- 关于应用 (README) 弹窗 -->
    <div v-if="showReadme" class="settings-modal-overlay" style="z-index: 100000;" @click.self="showReadme = false">
      <div class="readme-modal-content">
        <header class="settings-header">
          <h2>📖 关于应用 (README)</h2>
          <button class="close-btn" @click="showReadme = false">×</button>
        </header>
        <div class="readme-body">
  <h3>🚀 v1.1 版本更新</h3>
  <ul class="update-list">
    <li><strong>交互升级：</strong> 实现了丝滑的列表拖拽排序功能，增加物理按压反馈与幽灵悬浮动画。</li>
    <li><strong>视觉重构：</strong> 全新设计的系统设置与弹窗界面，深度适配现代毛玻璃与深色主题。</li>
    <li><strong>播放控制：</strong> 重新绘制音量控制滑块，加入随音量动态填充的视觉效果。</li>
    <li><strong>数据管理：</strong> 新增本地数据导出 JSON 备份、导入恢复以及一键恢复出厂设置功能。</li>
  </ul>
  
  <div class="github-link-wrapper">
    <p>查看更详细的使用指南、问题反馈或完整更新日志，请访问：</p>
    <!-- 请替换为你实际的 GitHub 仓库地址 -->
    <button class="github-link" @click="openGithub">
    🔗 前往 GitHub 主页
    </button>
  </div>
</div>
      </div>
    </div>

  </Teleport>
</template>

<style scoped>
/* 粘贴原 App.vue 中的 .modal-overlay, .cyber-modal, .custom-toast 相关 CSS */
.modal-overlay {
  position: fixed !important; top: 0; left: 0; right: 0; bottom: 0;
  z-index: 999999 !important;
  background: rgba(17, 17, 27, 0.7); backdrop-filter: blur(8px);
  display: flex !important; align-items: center; justify-content: center;
}
.cyber-modal {
  background: #181825; border: 1px solid #313244; border-radius: 12px; width: 420px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
}
.modal-header { padding: 16px 24px; border-bottom: 1px solid rgba(255, 255, 255, 0.05); display: flex; justify-content: space-between; }
.modal-header h3 { margin: 0; color: #f9e2af; font-size: 1.15rem; }
.close-btn { background: transparent; border: none; color: #a6adc8; cursor: pointer; font-size: 1.2rem; }
.modal-body { padding: 30px 24px; color: #cdd6f4; line-height: 1.8; }
.modal-actions { padding: 16px 24px; display: flex; justify-content: flex-end; gap: 16px; background: rgba(17, 17, 27, 0.5); }
.btn-secondary { background: rgba(255, 255, 255, 0.05); border: 1px solid rgba(255, 255, 255, 0.1); color: #cdd6f4; padding: 8px 16px; border-radius: 6px; cursor: pointer; }
.btn-exit { background: transparent; border: 1px solid #f38ba8; color: #f38ba8; padding: 10px 20px; border-radius: 6px; cursor: pointer; font-weight: 700; }
.custom-toast { position: fixed; top: 24px; left: 50%; transform: translateX(-50%); z-index: 9999; padding: 12px 24px; border-radius: 6px; background: rgba(30, 30, 36, 0.9); color: #e0e0e0; border: 1px solid rgba(255, 255, 255, 0.08); }
.custom-toast.success { border-left: 3px solid #5c8a60; }
.custom-toast.error { border-left: 3px solid #a35656; }
/* ================= 统一按钮样式 ================= */
.setting-actions {
  display: flex;
  gap: 12px;
  margin-top: 4px;
}

/* 基础按钮：带图标垂直居中 */
.btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  padding: 8px 16px;
  border-radius: 8px;
  border: 1px solid rgba(255, 255, 255, 0.1);
  background: rgba(255, 255, 255, 0.05);
  color: #cdd6f4;
  font-size: 0.9rem;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
}

.btn:hover {
  background: rgba(255, 255, 255, 0.1);
  transform: translateY(-1px);
}

.btn:active {
  transform: scale(0.96);
}

/* 次级按钮 (蓝色主题) */
.btn.secondary {
  background: rgba(137, 180, 250, 0.15);
  border-color: rgba(137, 180, 250, 0.3);
  color: #89b4fa;
}

.btn.secondary:hover {
  background: #89b4fa;
  color: #11111b;
}

/* 危险按钮 (红色主题) */
.btn.danger {
  background: rgba(243, 139, 168, 0.15);
  border-color: rgba(243, 139, 168, 0.3);
  color: #f38ba8;
}

.btn.danger:hover {
  background: #f38ba8;
  color: #11111b;
}

/* ================= 卡片式排版 ================= */
.settings-body {
  padding: 24px;
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 70vh;
  overflow-y: auto;
}

.setting-card {
  background: rgba(255, 255, 255, 0.02);
  border: 1px solid rgba(255, 255, 255, 0.04);
  border-radius: 10px;
  padding: 18px;
  display: flex;
  flex-direction: column;
  gap: 14px;
}

/* 危险区域特殊边框 */
.setting-card.danger-zone {
  border-color: rgba(243, 139, 168, 0.15);
  background: rgba(243, 139, 168, 0.03);
}

.setting-info h3 {
  font-size: 1.05rem;
  color: #cdd6f4;
  margin: 0 0 6px 0;
}

.setting-info p {
  font-size: 0.85rem;
  color: #a6adc8;
  margin: 0;
  line-height: 1.5;
}

.setting-actions {
  display: flex;
  gap: 10px;
}

/* ================= README 弹窗特殊样式 ================= */
.readme-modal-content {
  background: #1e1e2e; /* 稍亮一点的背景以区分设置面板 */
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 12px;
  width: 90%;
  max-width: 650px; /* 阅读区域需要更宽 */
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  box-shadow: 0 10px 40px rgba(0, 0, 0, 0.6);
}

.readme-body {
  padding: 24px;
  overflow-y: auto;
  color: #cdd6f4;
  font-size: 0.95rem;
  line-height: 1.6;
}
/* ================= 设置面板基础 ================= */
.settings-modal-overlay {
  position: fixed;
  top: 0; 
  left: 0; 
  right: 0; 
  bottom: 0;
  background: rgba(17, 17, 27, 0.8);
  backdrop-filter: blur(8px);
  display: flex; 
  align-items: center; 
  justify-content: center;
  z-index: 99999; /* 确保层级最高 */
}

.settings-modal-content {
  background: #181825;
  border: 1px solid rgba(255, 255, 255, 0.08);
  border-radius: 12px;
  width: 90%;
  max-width: 460px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.6);
  display: flex;
  flex-direction: column;
}

.settings-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 20px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.settings-header h2 {
  font-size: 1.2rem;
  color: #cdd6f4;
  margin: 0;
  font-weight: 600;
}

.close-btn {
  background: transparent;
  border: none;
  color: #a6adc8;
  font-size: 1.5rem;
  cursor: pointer;
  padding: 0;
  line-height: 1;
  transition: color 0.2s;
}

.close-btn:hover { 
  color: #f38ba8; 
}

.github-link {
  display: inline-flex; /* 垂直居中对齐图标和文字 */
  align-items: center;
  justify-content: center;
  padding: 10px 24px;
  background: rgba(137, 180, 250, 0.1);
  color: #89b4fa; /* 确保文字是纯正的主题蓝，不带默认链接色 */
  border-radius: 8px;
  border: 1px solid rgba(137, 180, 250, 0.3);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  font-weight: 600;
  font-size: 0.95rem;
  cursor: pointer; /* 确保鼠标悬停是小手 */
  outline: none; /* 去除按钮自带的焦点框 */
}

.github-link:hover {
  background: #89b4fa;
  color: #11111b;
  transform: translateY(-2px);
  box-shadow: 0 4px 12px rgba(137, 180, 250, 0.4);
}

.github-link:active {
  transform: scale(0.96);
}
</style>