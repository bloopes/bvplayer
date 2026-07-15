<script setup lang="ts">
import { ref, computed, nextTick, onBeforeUpdate, onBeforeUnmount } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import TrackItem from '../Common/TrackItem.vue';
import { usePlaylist } from '../../composables/usePlaylist';
import { usePlayer } from '../../composables/usePlayer'; 
import type { Song, ImportPayload } from '../../types';
import { useToast } from '../../composables/useToast';
import FadeTransition from '../Common/FadeTransition.vue';

// 从全局状态引入歌单数据
const { playlist, allPlaylists, activePlaylistId, saveDatabase } = usePlaylist();

const { showToast } = useToast();
const { currentSong } = usePlayer();
const currentView = ref<'library' | 'player'>('library');
const isDeleteMode = ref(false);
const newBvid = ref("");
const isImporting = ref(false);
const searchSongQuery = ref("");

// 📍 模糊搜索逻辑：保留歌曲在原歌单中的真实索引，避免搜索后删错歌
const filteredSongRows = computed(() => {
  const query = searchSongQuery.value.trim().toLowerCase();
  return playlist.value
    .map((song: Song, sourceIndex: number) => ({ song, sourceIndex }))
    .filter(({ song }) =>
      !query ||
      song.title.toLowerCase().includes(query) ||
      song.author.toLowerCase().includes(query)
    );
});

// 📍 拖拽排序与删除逻辑
const isSorting = ref(false);
const isPlaylistMutating = ref(false);
const suppressNextPlay = ref(false);
const draggedSongKey = ref("");
const pressedSongKey = ref("");
const songItemsRefs = ref<HTMLElement[]>([]);
let staticMidYPoints: number[] = [];
let cleanupSortListeners: (() => void) | null = null;
let suppressPlayTimer: ReturnType<typeof setTimeout> | null = null;

function suppressImmediatePlay() {
  suppressNextPlay.value = true;
  if (suppressPlayTimer) clearTimeout(suppressPlayTimer);
  suppressPlayTimer = setTimeout(() => {
    suppressNextPlay.value = false;
    suppressPlayTimer = null;
  }, 100);
}

function resetSongInteractions() {
  cleanupSortListeners?.();
  isDeleteMode.value = false;
  isSongSearchActive.value = false;
  searchSongQuery.value = "";
  isAddActive.value = false;
  newBvid.value = "";
}

// 📍 切换进入具体歌单
async function enterPlaylist(id: string) {
  if (!allPlaylists.value[id]) {
    showToast("歌单不存在或已被删除。", "error");
    return;
  }

  resetSongInteractions();
  activePlaylistId.value = id;
  currentView.value = 'player';
  await nextTick();

  try {
    await invoke("sync_playlist", { songs: playlist.value });
  } catch (err) {
    console.error(err);
    showToast(`歌单同步失败: ${String(err)}`, "error");
  }
}

function backToLibrary() {
  if (isImporting.value || isPlaylistMutating.value) return;
  resetSongInteractions();
  currentView.value = 'library';
}

onBeforeUpdate(() => {
  songItemsRefs.value = [];
});

onBeforeUnmount(() => {
  cleanupSortListeners?.();
  if (suppressPlayTimer) clearTimeout(suppressPlayTimer);
});

function getSongKey(song: Song) {
  return `${song.bvid}:${song.cid}`;
}

function setSongItemRef(el: unknown, index: number) {
  const element = (el as { $el?: HTMLElement } | null)?.$el ?? el;
  if (element instanceof HTMLElement) {
    songItemsRefs.value[index] = element;
  }
}

async function removeSongAtDisplayIndex(displayIndex: number) {
  if (isPlaylistMutating.value) return;

  const row = filteredSongRows.value[displayIndex];
  if (!row) return;

  suppressImmediatePlay();
  isPlaylistMutating.value = true;
  const removed = playlist.value.splice(row.sourceIndex, 1)[0];
  if (!removed) {
    isPlaylistMutating.value = false;
    return;
  }

  try {
    await invoke("sync_playlist", { songs: playlist.value });
    saveDatabase();
    showToast(`已删除《${removed.title}》`, "success");
  } catch (err) {
    playlist.value.splice(row.sourceIndex, 0, removed);
    console.error(err);
    showToast(`删除失败，已恢复歌曲: ${String(err)}`, "error");
  } finally {
    isPlaylistMutating.value = false;
  }
}

function handleItemAction(event: MouseEvent, displayIndex: number) {
  if (isImporting.value || isPlaylistMutating.value) return;
  if (event.button !== 0 && event.button !== 2) return;

  if (isDeleteMode.value) {
    if (event.button === 0) void removeSongAtDisplayIndex(displayIndex);
    return;
  }

  const row = filteredSongRows.value[displayIndex];
  const draggedSong = row?.song;
  if (!draggedSong || songItemsRefs.value.length === 0) return;

  cleanupSortListeners?.();

  const originalPlaylist = [...playlist.value];
  let currentDisplayIndex = displayIndex;
  let didReorder = false;

  // 1. 记录初始点击位置，设置防抖锁
  const startX = event.clientX;
  const startY = event.clientY;
  let isDraggingStarted = false;

  // 📍 新增：鼠标按下的瞬间，立刻记录“按下状态”
  pressedSongKey.value = getSongKey(draggedSong);

  // 获取初始的 Y 轴中心点
  staticMidYPoints = songItemsRefs.value.map((el) => {
    if (!el) return 0;
    const rect = el.getBoundingClientRect();
    return rect.top + rect.height / 2;
  });

  const onMouseMove = (moveEvent: MouseEvent) => {
    // 2. 拖拽阈值判断：移动距离超过 5px 才正式进入排序状态
    if (!isDraggingStarted) {
      const dist = Math.sqrt(Math.pow(moveEvent.clientX - startX, 2) + Math.pow(moveEvent.clientY - startY, 2));
      if (dist < 5) return; 

      isDraggingStarted = true;
      isSorting.value = true;
      draggedSongKey.value = getSongKey(draggedSong);
      document.body.classList.add('playlist-sorting');
    }

    // 仅在明确处于拖拽状态时，阻止默认的选中文本等行为
    moveEvent.preventDefault(); 

    if (staticMidYPoints.length < 2) return;

    let targetDisplayIndex = currentDisplayIndex;
    while (
      targetDisplayIndex < staticMidYPoints.length - 1 &&
      moveEvent.clientY > staticMidYPoints[targetDisplayIndex + 1]
    ) {
      targetDisplayIndex += 1;
    }
    while (
      targetDisplayIndex > 0 &&
      moveEvent.clientY < staticMidYPoints[targetDisplayIndex - 1]
    ) {
      targetDisplayIndex -= 1;
    }

    if (targetDisplayIndex === currentDisplayIndex) return;

    const targetSong = filteredSongRows.value[targetDisplayIndex]?.song;
    const currentSourceIndex = playlist.value.indexOf(draggedSong);
    const targetSourceIndex = targetSong ? playlist.value.indexOf(targetSong) : -1;
    if (currentSourceIndex < 0 || targetSourceIndex < 0) return;

    playlist.value.splice(currentSourceIndex, 1);
    playlist.value.splice(targetSourceIndex, 0, draggedSong);
    currentDisplayIndex = targetDisplayIndex;
    didReorder = true;
    suppressImmediatePlay?.();

    // 3. 更新坐标系：DOM 重排后，重新获取静止元素的 Y 轴中心点
    // 设置 50ms 延迟以确保 Vue 完成 DOM 更新
    setTimeout(() => {
      staticMidYPoints = songItemsRefs.value.map((el) => {
        if (!el) return 0;
        const rect = el.getBoundingClientRect();
        return rect.top + rect.height / 2;
      });
    }, 50);
  };

  const finishSorting = async () => {
    cleanupSortListeners?.();
    cleanupSortListeners = null;

    // 4. 如果只点击没拖拽，或拖拽后位置没变，直接跳过数据保存
    if (!isDraggingStarted || !didReorder) return;
    
    isPlaylistMutating.value = true;

    try {
      await invoke("sync_playlist", { songs: playlist.value });
      saveDatabase();
    } catch (err) {
      playlist.value.splice(0, playlist.value.length, ...originalPlaylist);
      console.error(err);
      showToast(`排序保存失败，已恢复原顺序: ${String(err)}`, "error");
    } finally {
      isPlaylistMutating.value = false;
    }
  };

  const onMouseUp = () => {
    void finishSorting();
  };

  cleanupSortListeners = () => {
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    document.body.classList.remove('playlist-sorting');
    isSorting.value = false;
    draggedSongKey.value = "";
    pressedSongKey.value = "";
  };

  window.addEventListener('mousemove', onMouseMove, { passive: false });
  window.addEventListener('mouseup', onMouseUp, { once: true });
}

// 📍 播放指令
async function playSong(index: number) {
  if (suppressNextPlay.value) {
    suppressNextPlay.value = false;
    return;
  }
  if (isImporting.value || isPlaylistMutating.value) return;

  const row = filteredSongRows.value[index];
  if (!row) return;

  try {
    await invoke("sync_playlist", { songs: playlist.value });
    await invoke("play_at_index", { index: row.sourceIndex });
  } catch (err) {
    console.error(err);
    showToast(`播放失败: ${String(err)}`, "error");
  }
}

// 📍 导歌漏斗逻辑 (原样保留智能判定)
async function addSong() {
  if (isImporting.value || isPlaylistMutating.value || !newBvid.value.trim()) return;
  const inputVal = newBvid.value.trim();
  isImporting.value = true;
  let shouldClearInput = false;

  try {
    let commandName = 'import_bvid';
    let payload: ImportPayload = {};

    const seasonMatch = inputVal.match(/season_id=(\d+)/i);
    const favMatch = inputVal.match(/fid=(\d+)/i) || inputVal.match(/^(\d{5,})$/);
    const bvMatch = inputVal.match(/(BV1[A-Za-z0-9]{9})/i);

    if (seasonMatch) {
      commandName = 'import_season_list';
      payload = { sid: seasonMatch[1] };
    } else if (favMatch) {
      commandName = 'import_fav_list';
      payload = { fid: favMatch[1] };
    } else {
      payload = { bvid: bvMatch ? bvMatch[1] : inputVal };
    }

    const fetchedSongs = await invoke<Song[]>(commandName, payload);
    if (!Array.isArray(fetchedSongs)) {
      throw new Error("后端返回的数据格式无效");
    }

    const uniqueSongs = fetchedSongs.filter((newSong: Song) =>
      !playlist.value.some((song: Song) =>
        song.bvid === newSong.bvid && song.cid === newSong.cid
      )
    );

    if (uniqueSongs.length > 0) {
      const originalLength = playlist.value.length;
      playlist.value.push(...uniqueSongs);
      try {
        await invoke("sync_playlist", { songs: playlist.value });
        saveDatabase();
      } catch (err) {
        playlist.value.splice(originalLength, uniqueSongs.length);
        throw err;
      }
      showToast(`导入完成，新增 ${uniqueSongs.length} 首歌曲。`, 'success');
    } else {
      showToast("当前歌单中已存在这些歌曲。", 'warning');
    }

    shouldClearInput = true;
  } catch (err) {
    console.error(err);
    showToast(`导入失败: ${String(err)}`, 'error');
  } finally {
    isImporting.value = false;
    if (shouldClearInput) {
      newBvid.value = "";
      isAddActive.value = false;
    } else {
      await nextTick();
      addInputRef.value?.focus();
    }
  }
}
// 📍 恢复：歌单界面的模糊搜索逻辑
const searchPlaylistQuery = ref("");
const isLibSearchActive = ref(false);
const libSearchInputRef = ref<HTMLInputElement | null>(null);

const filteredPlaylists = computed(() => {
  const playlistsArray = Object.values(allPlaylists.value);
  if (!searchPlaylistQuery.value.trim()) return playlistsArray;
  const query = searchPlaylistQuery.value.toLowerCase();
  return playlistsArray.filter(list => list.name.toLowerCase().includes(query));
});

async function activateLibSearch() {
  isLibSearchActive.value = true;
  await nextTick();
  libSearchInputRef.value?.focus();
}
function onLibSearchBlur() {
  if (!searchPlaylistQuery.value.trim()) isLibSearchActive.value = false;
}

// 📍 恢复：折叠式新建歌单逻辑
const newPlaylistName = ref("");
const isCreateActive = ref(false);
const createInputRef = ref<HTMLInputElement | null>(null);

function createNewPlaylist() {
  const name = newPlaylistName.value.trim();
  if (!name) return;

  const duplicate = Object.values(allPlaylists.value).some(
    (item) => item.name.trim().toLowerCase() === name.toLowerCase()
  );
  if (duplicate) {
    showToast("已存在同名歌单。", "warning");
    return;
  }

  const id = `pl_${Date.now()}_${Math.random().toString(36).slice(2, 7)}`;
  allPlaylists.value[id] = {
    id,
    name,
    cover: "https://i0.hdslb.com/bfs/archive/478a05f0134440062b1aab93d62957bda52b57e9.jpg",
    songs: []
  };
  saveDatabase();
  newPlaylistName.value = "";
  isCreateActive.value = false;
  showToast("歌单已创建。", "success");
}

async function activateCreate() {
  if (isCreateActive.value && newPlaylistName.value.trim()) {
    createNewPlaylist();
    isCreateActive.value = false;
    return;
  }
  isCreateActive.value = true;
  await nextTick();
  createInputRef.value?.focus();
}
function onCreateBlur() {
  if (!newPlaylistName.value.trim()) isCreateActive.value = false;
}
// 📍 触发全局设置弹窗
function openSettings() { window.dispatchEvent(new CustomEvent('open-settings')); }

// 📍 恢复 1：封面本地自存储引擎 (Canvas 压缩转 Base64)
const coverInputRef = ref<HTMLInputElement | null>(null);
const targetPlaylistIdForCover = ref("");

function changePlaylistCover(id: string) {
  targetPlaylistIdForCover.value = id;
  if (coverInputRef.value) coverInputRef.value.click();
}

function onCoverFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  const file = input.files?.[0];
  input.value = "";
  if (!file) return;

  if (!file.type.startsWith("image/")) {
    showToast("请选择有效的图片文件。", "warning");
    return;
  }

  const targetId = targetPlaylistIdForCover.value;
  const reader = new FileReader();

  reader.onerror = () => showToast("读取封面文件失败。", "error");
  reader.onload = (e) => {
    const result = e.target?.result;
    if (typeof result !== "string") return;

    const img = new Image();
    img.onerror = () => showToast("图片解析失败，请更换文件。", "error");
    img.onload = () => {
      const canvas = document.createElement("canvas");
      const MAX_SIZE = 400;
      const scale = Math.min(1, MAX_SIZE / Math.max(img.width, img.height));
      const width = Math.max(1, Math.round(img.width * scale));
      const height = Math.max(1, Math.round(img.height * scale));

      canvas.width = width;
      canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (!ctx) {
        showToast("当前环境无法处理图片。", "error");
        return;
      }

      ctx.drawImage(img, 0, 0, width, height);
      const targetPlaylist = allPlaylists.value[targetId];
      if (!targetPlaylist) {
        showToast("目标歌单不存在。", "error");
        return;
      }

      targetPlaylist.cover = canvas.toDataURL("image/jpeg", 0.8);
      saveDatabase();
      showToast("封面更新成功。", "success");
    };
    img.src = result;
  };
  reader.readAsDataURL(file);
}

// 📍 恢复 2：删除歌单逻辑
function deletePlaylist(id: string) {
  if (id === 'default') {
    showToast("默认歌单为系统保留，不可删除。", "warning");
    return;
  }
  if (!allPlaylists.value[id]) return;
  if (!confirm("确定要永久删除该歌单吗？")) return;

  delete allPlaylists.value[id];
  if (activePlaylistId.value === id) {
    activePlaylistId.value = allPlaylists.value.default
      ? 'default'
      : Object.keys(allPlaylists.value)[0] ?? '';
  }
  saveDatabase();
  showToast("歌单已删除。", "success");
}
// 📍 恢复：歌曲界面的搜索折叠逻辑
const isSongSearchActive = ref(false);
const songSearchInputRef = ref<HTMLInputElement | null>(null);

async function activateSongSearch() {
  isSongSearchActive.value = true;
  await nextTick();
  songSearchInputRef.value?.focus();
}

function onSongSearchBlur() {
  if (!searchSongQuery.value.trim()) {
    isSongSearchActive.value = false;
  }
}

// 📍 恢复：歌曲界面的添加折叠逻辑
const isAddActive = ref(false);
const addInputRef = ref<HTMLInputElement | null>(null);

async function activateAdd() {
  if (isImporting.value || isPlaylistMutating.value) return; 

  // 如果已经展开并且输入了链接，再次点击 ➕ 号相当于点击"确认注入"
  if (isAddActive.value && newBvid.value.trim()) {
    await addSong();
    isAddActive.value = false; 
    return;
  }

  isAddActive.value = true;
  await nextTick();
  addInputRef.value?.focus();
}

function onAddBlur() {
  if (!newBvid.value.trim()) {
    isAddActive.value = false;
  }
}
</script>

<template>
  <div class="main-viewport">
    <!-- 📍 隐藏的封面上传控件 (不参与过渡动画) -->
    <input type="file" ref="coverInputRef" accept="image/png, image/jpeg, image/webp" style="display: none;" @change="onCoverFileSelected" />
    
    <!-- 📍 加入过渡组件包裹 -->
    <FadeTransition>
      
      <!-- 视图 A：大厅界面 -->
      <div v-if="currentView === 'library'" class="library-view">
        <header class="lib-header">
          <h1>🗃️ 歌单列表</h1>
          <div class="create-box">
            <!-- 搜索歌单 -->
            <div class="expandable-search" :class="{ 'is-active': isLibSearchActive || searchPlaylistQuery }">
              <button class="btn-icon" @click="activateLibSearch" title="搜索歌单">🔍</button>
              <input
                ref="libSearchInputRef" type="text" v-model="searchPlaylistQuery"
                placeholder="搜索歌单..." class="search-input-hidden" @blur="onLibSearchBlur"
              />
            </div>

            <!-- 新建歌单 -->
            <div class="expandable-search" :class="{ 'is-active': isCreateActive || newPlaylistName }">
              <button class="btn-icon" @click="activateCreate" title="新建歌单">➕</button>
              <input
                ref="createInputRef" type="text" v-model="newPlaylistName"
                placeholder="输入新名称..." class="search-input-hidden"
                @blur="onCreateBlur" @keydown.enter.prevent="createNewPlaylist"
              />
            </div>

            <!-- 设置按钮回归 -->
            <button @click="openSettings" class="btn-icon" title="系统设置">⚙️</button>
          </div>
        </header>

        <div v-if="filteredPlaylists.length > 0" class="playlist-grid">
          <div v-for="pl in filteredPlaylists" :key="pl.id" class="pl-card" @click="enterPlaylist(pl.id)">
            <div class="pl-cover-wrapper">
              <img :src="pl.cover" referrerpolicy="no-referrer" class="pl-cover" />
              
              <!-- 📍 操作按钮组 -->
              <div class="pl-actions">
                <button @click.stop="changePlaylistCover(pl.id)" class="btn-small" title="本地选择封面">🖼️</button>
                <button v-if="pl.id !== 'default'" @click.stop="deletePlaylist(pl.id)" class="btn-small danger" title="删除歌单">🗑️</button>
              </div>
              
            </div>
            <div class="pl-info">
              <div class="pl-name">{{ pl.name }}</div>
              <div class="pl-count">{{ pl.id === activePlaylistId ? playlist.length : (pl.songs?.length || 0) }} 首母带</div>
            </div>
          </div>
        </div>
        <div v-else class="empty-state">没有找到匹配的歌单</div>
      </div>

      <!-- 视图 B：单曲列表界面 -->
      <div v-else class="queue-panel">
        <header class="detail-header">
          <button
            @click="backToLibrary"
            class="btn-back"
            :disabled="isImporting || isPlaylistMutating"
          >⬅ 返回</button>
          <h2>{{ allPlaylists[activePlaylistId]?.name }} ({{ playlist.length }})</h2>

          <div class="input-group">
            <!-- 搜索歌曲框 -->
            <div class="expandable-search" :class="{ 'is-active': isSongSearchActive || searchSongQuery }">
              <button class="btn-icon" @click="activateSongSearch" title="搜索歌曲">🔍</button>
              <input
                ref="songSearchInputRef"
                type="text"
                v-model="searchSongQuery"
                placeholder="搜索歌曲或UP主..."
                class="search-input-hidden"
                @blur="onSongSearchBlur"
              />
            </div>

            <!-- 添加歌曲框 -->
            <div class="expandable-search" :class="{ 'is-active': isAddActive || newBvid }">
              <button
                class="btn-icon"
                @click="activateAdd"
                title="添加歌曲 (点击或回车确认)"
                :disabled="isImporting || isPlaylistMutating"
              >
                {{ isImporting ? '⏳' : '➕' }}
              </button>
              <input
                ref="addInputRef"
                type="text"
                v-model="newBvid"
                placeholder="输入 BV/链接 (回车)"
                class="search-input-hidden"
                @blur="onAddBlur"
                @keydown.enter.prevent="addSong"
                :disabled="isImporting || isPlaylistMutating"
              />
            </div>

            <!-- 删除模式开关 -->
            <button
              @click="isDeleteMode = !isDeleteMode"
              class="btn-icon danger-toggle"
              :class="{ 'active': isDeleteMode }"
              :disabled="isImporting || isPlaylistMutating"
              title="切换删除模式"
            >
              {{ isDeleteMode ? '🛑' : '🗑️' }}
            </button>
          </div>
        </header>

        <TransitionGroup 
          name="list" 
          tag="ul" 
          v-if="filteredSongRows.length > 0" 
          :class="['song-list', { 'delete-mode': isDeleteMode }]"
        >
          <TrackItem
            v-for="(row, index) in filteredSongRows"
            :key="getSongKey(row.song)"
            :ref="el => setSongItemRef(el, index)"
            :song="row.song"
            :index="index"
            :isActive="currentSong?.bvid === row.song.bvid && currentSong?.cid === row.song.cid"
            :isDragging="isSorting && draggedSongKey === getSongKey(row.song)"
            :isDeleteMode="isDeleteMode"
            :isPressed="pressedSongKey === getSongKey(row.song)" 
            @play="playSong"
            @drag-start="handleItemAction"
            @contextmenu.prevent
          />
        </TransitionGroup>
        <div v-else-if="playlist.length === 0" class="empty-state">歌单空空如也，请在上方填入视频链接或 BV 号</div>
        <div v-else class="empty-state">没有找到匹配的歌曲</div>
      </div>

    </FadeTransition>
  </div>
</template>

<style scoped>
/* 包含原 App.vue 中关于 .main-viewport, .library-view, .queue-panel 等所有布局 CSS */
.main-viewport { flex: 1; overflow-y: auto; display: flex; flex-direction: column; }
.library-view { padding: 35px 40px; }
.lib-header { display: flex; justify-content: space-between; align-items: center; margin-bottom: 35px; }
.lib-header h1 { font-size: 1.6rem; color: #cdd6f4; font-weight: 700; margin: 0; }
.playlist-grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(220px, 1fr)); gap: 25px; }
.pl-card { background: #181825; border-radius: 12px; overflow: hidden; cursor: pointer; transition: all 0.3s ease; border: 1px solid rgba(255, 255, 255, 0.03); }
.pl-card:hover { transform: translateY(-6px); border-color: rgba(137, 180, 250, 0.4); }
.pl-cover-wrapper { position: relative; width: 100%; aspect-ratio: 1; background: #1e1e2e; overflow: hidden; }
.pl-cover { width: 100%; height: 100%; object-fit: cover; transition: transform 0.5s ease; }
.pl-card:hover .pl-cover { transform: scale(1.05); filter: brightness(0.7); }
.pl-info { padding: 18px; }
.pl-name { color: #cdd6f4; font-weight: 600; font-size: 1.05rem; margin-bottom: 6px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.pl-count { color: #6c7086; font-size: 0.85rem; }

.queue-panel { padding: 25px 40px; display: flex; flex-direction: column; height: 100%; }
.detail-header { display: flex; align-items: center; justify-content: space-between; margin-bottom: 25px; padding-bottom: 25px; border-bottom: 1px solid rgba(255, 255, 255, 0.05); }
.btn-back { background: transparent; border: 1px solid #45475a; color: #a6adc8; padding: 8px 16px; border-radius: 6px; cursor: pointer; }
.btn-back:disabled { opacity: 0.55; cursor: not-allowed; }
.detail-header h2 { color: #cdd6f4; margin: 0 0 0 20px; font-size: 1.4rem; flex: 1; }
.input-group { display: flex; gap: 12px; }
.search-box, .add-box { background: rgba(24, 24, 37, 0.8); border: 1px solid #313244; color: #cdd6f4; padding: 8px 14px; border-radius: 6px; outline: none; }
.search-box:focus, .add-box:focus { border-color: #89b4fa; }

.song-list { list-style: none; display: flex; flex-direction: column; gap: 6px; padding: 0; margin: 0; padding-bottom: 20px; }
.danger-toggle { background: transparent; border: none; font-size: 1.2rem; cursor: pointer; border-radius: 6px; padding: 4px 8px; }
.danger-toggle.active { background: #f38ba8; color: #11111b; }
.danger-toggle:disabled, .btn-icon:disabled { opacity: 0.55; cursor: not-allowed; }
.empty-state { text-align: center; color: #585b70; margin-top: 60px; font-size: 1.1rem; }
/* 📍 恢复：折叠搜索框与图标按钮样式 */
.create-box { display: flex; gap: 12px; }

.btn-icon {
  background: transparent; border: none; color: #a6adc8;
  padding: 10px; border-radius: 8px; font-size: 1.1rem; cursor: pointer;
}
.btn-icon:hover { background: rgba(255, 255, 255, 0.05); color: #cdd6f4; }

.expandable-search {
  display: flex; align-items: center; background: transparent;
  border-radius: 6px; border: 1px solid transparent;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  overflow: hidden; height: 38px;
}
.expandable-search.is-active {
  background-color: rgba(30, 30, 36, 0.6);
  border-color: rgba(255, 255, 255, 0.1);
}
.expandable-search .btn-icon { margin: 0; padding: 8px; height: 100%; }

.search-input-hidden {
  width: 0; padding: 0; border: none; background: transparent;
  color: #e0e0e0; outline: none; font-size: 0.9rem;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1), padding 0.3s ease;
}
.expandable-search.is-active .search-input-hidden {
  width: 160px; padding: 0 12px 0 4px;
}
.search-input-hidden::placeholder { color: rgba(255, 255, 255, 0.3); }
/* 📍 恢复：封面上的悬浮操作按钮组 */
.pl-actions {
  position: absolute;
  top: 12px;
  right: 12px;
  display: flex;
  gap: 8px;
  opacity: 0;
  transform: translateY(-10px);
  transition: all 0.2s ease;
}

.pl-card:hover .pl-actions,
.pl-card:focus-within .pl-actions {
  opacity: 1;
  transform: translateY(0);
}

.btn-small {
  background: rgba(17, 17, 27, 0.7);
  backdrop-filter: blur(4px);
  border: 1px solid rgba(255, 255, 255, 0.1);
  border-radius: 6px;
  padding: 6px;
  cursor: pointer;
  font-size: 14px;
  color: #fff;
}

.btn-small:hover {
  background: #89b4fa;
  border-color: #89b4fa;
}

.btn-small.danger:hover {
  background: #f38ba8;
  border-color: #f38ba8;
}

/* 📍 真正的幽灵悬浮状态 */
.track-item.is-dragging {
  opacity: 0.85; /* 透明度调高一点，保证看清内容 */
  transform: scale(1.02); /* 微微放大，产生“被提起来”的空间感 */
  background: rgba(137, 180, 250, 0.15) !important;
  border: 1px dashed rgba(137, 180, 250, 0.8);
  box-shadow: 0 12px 24px rgba(0, 0, 0, 0.4); /* 增加底部深邃阴影，强化悬浮感 */
  cursor: grabbing;
  z-index: 100; /* 强行置顶 */
}

/* 📍 列表增加缓动 */
.song-list {
  transition: transform 0.2s ease;
}

:global(body.playlist-sorting) {
  user-select: none;
  cursor: grabbing;
}

@media (hover: none) {
  .pl-actions {
    opacity: 1;
    transform: none;
  }
}

/* 📍 Vue FLIP 动画引擎：让列表换位变得像丝一样顺滑 */
.list-move {
  transition: transform 0.35s cubic-bezier(0.2, 0, 0, 1);
}

/* 确保拖拽中的元素层级最高，不会被其他滑动的元素遮挡 */
.song-list {
  position: relative;
}
</style>