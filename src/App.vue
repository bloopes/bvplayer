<script setup lang="ts">
/**
 * bvplayer 主界面组件（单文件组件 SFC）。
 *
 * 整个应用的前端逻辑集中于此，采用 Vue 3 Composition API（script setup）。
 *
 * ## 架构决策
 *
 * - **单组件聚合**：所有状态（歌单、播放、UI）集中管理，避免过早拆分
 *   导致的 props/emits 传递复杂度。待规模进一步膨胀后再按功能域拆分为子组件。
 * - **localStorage 持久化**：播放数据量通常在 KB~MB 级，同步读写简化了
 *   初始化流程，且无需引入 IndexedDB 或后端 SQLite。
 * - **Tauri IPC 双向通信**：前端通过 invoke() 调用 Rust 后端 command，
 *   后端通过 emit() 推送事件（playback-start / playback-progress / toggle-exit-modal）。
 *   Rust 侧持有播放列表的权威状态（PlaylistManager），前端仅作 UI 镜像缓存。
 * - **手动类型声明**：Song/PlaylistMeta 接口在后端由 Rust serde 序列化，
 *   前端手动维护对应的 TS 接口。生产化时应引入 ts-rs 自动生成以消除同步风险。
 */
import { computed ,nextTick} from 'vue'; // 确保顶部引入了 computed

// ==========================================
// 模糊搜索：
// 歌曲搜索（searchSongQuery）和歌单搜索（searchPlaylistQuery）各自维护独立的
// 折叠/展开状态与 DOM 引用，避免两个搜索框的交互互相干扰。
// ==========================================

// 🔍 1. 歌曲界面的模糊搜索
// ==========================================
const searchSongQuery = ref("");

// 之后界面渲染和播放同步，都用这个 filteredSongs，而不是原来的 playlist
const filteredSongs = computed(() => {
  if (!searchSongQuery.value.trim()) return playlist.value; // 没输入时显示全部

  const query = searchSongQuery.value.toLowerCase();
  return playlist.value.filter(song =>
    song.title.toLowerCase().includes(query) ||
    song.author.toLowerCase().includes(query)
  );
});


// ==========================================
// 🔍 2. 歌单界面的模糊搜索
// ==========================================
const searchPlaylistQuery = ref("");

const filteredPlaylists = computed(() => {
  // 📍 核心修复：把 Record 对象转换为 Array 数组
  const playlistsArray = Object.values(allPlaylists.value);

  if (!searchPlaylistQuery.value.trim()) return playlistsArray;

  const query = searchPlaylistQuery.value.toLowerCase();
  return playlistsArray.filter(list =>
    list.name.toLowerCase().includes(query)
  );
});

// ==========================================
// ➕ 折叠式新建歌单逻辑
// ==========================================
const isCreateActive = ref(false);
const createInputRef = ref<HTMLInputElement | null>(null);

async function activateCreate() {
  // 如果已经展开并且输入了名字，再次点击 ➕ 号相当于点击"确认创建"
  if (isCreateActive.value && newPlaylistName.value.trim()) {
    createNewPlaylist();
    isCreateActive.value = false; // 创建完自动收起
    return;
  }

  isCreateActive.value = true;
  await nextTick();
  createInputRef.value?.focus();
}

function onCreateBlur() {
  // 如果没输入东西就点别处，自动收起
  if (!newPlaylistName.value.trim()) {
    isCreateActive.value = false;
  }
}

// ==========================================
// 🔍 折叠搜索框交互逻辑
// ==========================================

// 1. 歌单界面的搜索状态
const isLibSearchActive = ref(false);
const libSearchInputRef = ref<HTMLInputElement | null>(null);

async function activateLibSearch() {
  isLibSearchActive.value = true;
  await nextTick(); // 等待输入框 DOM 渲染完毕
  libSearchInputRef.value?.focus(); // 自动获取焦点
}

function onLibSearchBlur() {
  // 只有在没输入任何字的情况下失去焦点，才会自动收缩
  if (!searchPlaylistQuery.value.trim()) {
    isLibSearchActive.value = false;
  }
}

// 2. 歌曲界面的搜索状态
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

// ==========================================
// 音量管理：
// 使用平方曲线（getPhysicalVolume）拟合人耳对响度的对数感知特性。
// UI 线性滑块 → 物理指数音量：0.5 的 UI 值输出 0.25 物理音量，
// 使滑块在低音量区域有更细腻的调节精度。
// 音量值持久化到 localStorage，启动时恢复；静音前记忆 previousVolume 供恢复。
// ==========================================
// 📍 音量状态 (带有指数曲线与持久化记忆)
// ==========================================
const savedVolume = localStorage.getItem('b-player-volume');
// 强制声明为 number 类型
const currentVolume = ref<number>(savedVolume !== null ? parseFloat(savedVolume) : 1.0);
const isMuted = ref(currentVolume.value === 0);
const previousVolume = ref<number>(1.0); // 用于静音恢复

// 📍 将 UI 线性音量转换为物理指数音量
function getPhysicalVolume(uiVol: number): number {
  // 采用平方曲线拟合人耳听觉：0.5 的 UI 音量会输出 0.25 的物理音量，听感更平滑
  return Math.pow(uiVol, 2);
}

function updateVolume() {
  // 强制转为数字，防御 HTML input 的字符串转换
  const val = Number(currentVolume.value);
  isMuted.value = val === 0;

  localStorage.setItem('b-player-volume', val.toString());

  // 发送计算后的物理音量给 Rust
  invoke("set_volume", { vol: getPhysicalVolume(val) }).catch(err => {
    console.error("音量调节过快:", err);
  });
}

async function toggleMute() {
  if (isMuted.value) {
    currentVolume.value = previousVolume.value > 0 ? previousVolume.value : 1.0;
  } else {
    previousVolume.value = Number(currentVolume.value);
    currentVolume.value = 0;
  }
  updateVolume();
}

// ==========================================
// ➕ 折叠式注入/添加逻辑
// ==========================================
const isAddActive = ref(false);
const addInputRef = ref<HTMLInputElement | null>(null);

async function activateAdd() {
  if (isImporting.value) return; // 正在解析时拦截

  // 如果已经展开并且输入了链接，再次点击 ➕ 号相当于点击"确认注入"
  if (isAddActive.value && newBvid.value.trim()) {
    await addSong();
    isAddActive.value = false; // 注入后自动收起
    return;
  }

  isAddActive.value = true;
  await nextTick();
  addInputRef.value?.focus();
}

function onAddBlur() {
  // 如果没输入东西就点别处，自动收起
  if (!newBvid.value.trim()) {
    isAddActive.value = false;
  }
}

// 📍 Toast 轻提示状态
const toast = ref({
  visible: false,
  message: '',
  type: 'success' // 可选值：'success' | 'warning' | 'error'
});

let toastTimer: ReturnType<typeof setTimeout>;

// 📍 触发 Toast 的通用函数
function showToast(message: string, type: 'success' | 'warning' | 'error' = 'success') {
  toast.value.message = message;
  toast.value.type = type;
  toast.value.visible = true;

  // 每次触发前清除上一次的定时器，防止连续点击导致闪烁
  if (toastTimer) clearTimeout(toastTimer);

  // 3秒后自动消失
  toastTimer = setTimeout(() => {
    toast.value.visible = false;
  }, 3000);
}

import { ref, onMounted, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { save } from '@tauri-apps/plugin-dialog';

// 提取出一个公共函数：根据鼠标的 X 坐标计算并更新 currentProgress
function calculateProgressFromX(clientX: number) {
  if (!progressBarRef.value || !currentSong.value) return;

  const rect = progressBarRef.value.getBoundingClientRect();
  let offsetX = clientX - rect.left;

  // 边界安全处理：防止鼠标移出进度条左侧或右侧时算错时间
  if (offsetX < 0) offsetX = 0;
  if (offsetX > rect.width) offsetX = rect.width;

  const percentage = offsetX / rect.width;
  currentProgress.value = percentage * currentSong.value.duration;
}

// ==========================================
// 🗂️ 核心数据结构与状态机
// ==========================================
interface Song { bvid: string; title: string; author: string; cover_url: string; audio_url: string; duration: number; cid: number;}

interface PlaylistMeta {
  id: string;
  name: string;
  cover: string;
  songs: Song[];
}

const currentView = ref<'library' | 'player'>('library');
const allPlaylists = ref<Record<string, PlaylistMeta>>({});
const activePlaylistId = ref<string>("default");
const playlist = ref<Song[]>([]);

const currentSong = ref<Song | null>(null);
const currentProgress = ref(0);
const isPlaying = ref(false);
const currentMode = ref("ListLoop");
const modeIcons: Record<string, string> = { "ListLoop": "🔁", "SingleLoop": "🔂", "Shuffle": "🔀" };
const modeTitles: Record<string, string> = { "ListLoop": "列表循环", "SingleLoop": "单曲循环", "Shuffle": "随机播放" };

const newBvid = ref("");
const newPlaylistName = ref("");
const isDeleteMode = ref(false);
const progressBarRef = ref<HTMLElement | null>(null);
const isDragging = ref(false);
const seekLock = ref(false); // 📍 新增：跳转霸体锁

// ==========================================
// 🖼️ 封面本地自存储引擎：
// 将用户选择的本地图片压缩为 JPEG base64 并存入 allPlaylists，
// 避免依赖外部 CDN（B站封面可能过期或跨域限制）。
// 压缩策略：最大 400px 宽/高，JPEG 质量 0.8，在视觉可接受的前提下最小化 localStorage 占用。
// ==========================================
const coverInputRef = ref<HTMLInputElement | null>(null);
const targetPlaylistIdForCover = ref("");

function changePlaylistCover(id: string, event: Event) {
  event.stopPropagation();
  targetPlaylistIdForCover.value = id;
  if (coverInputRef.value) coverInputRef.value.click();
}

function onCoverFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) return;

  const file = input.files[0];
  const reader = new FileReader();

  reader.onload = (e) => {
    const result = e.target?.result as string;
    const img = new Image();
    img.onload = () => {
      const canvas = document.createElement("canvas");
      const MAX_SIZE = 400;
      let width = img.width;
      let height = img.height;

      if (width > height) {
        if (width > MAX_SIZE) { height *= MAX_SIZE / width; width = MAX_SIZE; }
      } else {
        if (height > MAX_SIZE) { width *= MAX_SIZE / height; height = MAX_SIZE; }
      }

      canvas.width = width; canvas.height = height;
      const ctx = canvas.getContext("2d");
      if (ctx) {
        ctx.drawImage(img, 0, 0, width, height);
        const compressedBase64 = canvas.toDataURL("image/jpeg", 0.8);
        if (targetPlaylistIdForCover.value && allPlaylists.value[targetPlaylistIdForCover.value]) {
          allPlaylists.value[targetPlaylistIdForCover.value].cover = compressedBase64;
          saveDatabase();
        }
      }
    };
    img.src = result;
  };
  reader.readAsDataURL(file);
  input.value = "";
}

// ==========================================
// 💾 数据持久化：
// 将整个 allPlaylists 对象 JSON 序列化后存入 localStorage。
// 通过 watch(playlist, deep) 自动同步播放列表变化到持久层，
// audio_url 字段在序列化前主动清空，避免保存已过期的防盗链签名 URL。
// ==========================================
async function initDatabase() {
  const dbData = localStorage.getItem('b-player-database');
  if (dbData) {
    allPlaylists.value = JSON.parse(dbData);
  } else {
    const oldData = localStorage.getItem('b-player-playlist');
    const initialSongs = oldData ? JSON.parse(oldData) : [];
    allPlaylists.value = {
      "default": {
        id: "default",
        name: "默认歌单",
        cover: "https://i0.hdslb.com/bfs/archive/478a05f0134440062b1aab93d62957bda52b57e9.jpg",
        songs: initialSongs
      }
    };
    saveDatabase();
  }
}

function saveDatabase() {
  localStorage.setItem('b-player-database', JSON.stringify(allPlaylists.value));
}

watch(playlist, (newList) => {
  if (activePlaylistId.value && allPlaylists.value[activePlaylistId.value]) {
    allPlaylists.value[activePlaylistId.value].songs = newList.map(song => ({ ...song, audio_url: "" }));
    saveDatabase();
  }
}, { deep: true });

// ==========================================
// 🚪 界面与控制交互
// ==========================================
async function enterPlaylist(id: string) {
  activePlaylistId.value = id;
  playlist.value = allPlaylists.value[id].songs;
  songItemsRefs.value = [];
  currentView.value = 'player';
  await invoke("sync_playlist", { songs: playlist.value });
}

function createNewPlaylist() {
  const name = newPlaylistName.value.trim();
  if (!name) return;
  const id = "pl_" + Date.now();
  allPlaylists.value[id] = {
    id, name,
    cover: "https://i0.hdslb.com/bfs/archive/478a05f0134440062b1aab93d62957bda52b57e9.jpg",
    songs: []
  };
  newPlaylistName.value = "";
  saveDatabase();
}

function deletePlaylist(id: string, event: Event) {
  event.stopPropagation();
  if (id === 'default') { showToast("默认歌单为系统保留，不可删除！"); return; }
  if (confirm("确定要永久删除该歌单吗？")) { delete allPlaylists.value[id]; saveDatabase(); }
}

async function removeSong(index: number) {
  playlist.value.splice(index, 1);
  await invoke("sync_playlist", { songs: playlist.value });
}

const audioDevices = ref<string[]>([]);
const selectedDevice = ref<string>("");

async function loadAudioDevices() {
  try {
    audioDevices.value = await invoke("get_audio_devices");
    const savedDevice = localStorage.getItem('b-player-device');
    if (savedDevice && audioDevices.value.includes(savedDevice)) {
      selectedDevice.value = savedDevice;
      await invoke("set_audio_device", { device: savedDevice });
    }
  } catch (e) { console.error("硬件枚举失败:", e); }
}

async function changeDevice() {
  localStorage.setItem('b-player-device', selectedDevice.value);
  await invoke("set_audio_device", { device: selectedDevice.value });
  await invoke("switch_device_realtime", { device: selectedDevice.value });
}

const isSorting = ref(false);
const dragIndex = ref<number | null>(null);
const songItemsRefs = ref<HTMLElement[]>([]);

let staticMidYPoints: number[] = [];

/**
 * 右键拖拽排序：
 * 使用右键（button=2）作为拖拽触发器，避免与左键的文本选择和双击播放冲突。
 * swapLock 冷却锁（150ms）防止快速拖动时过度交换导致 UI 抖动；
 * 每次交换后重新采样各列表项的 Y 轴中点坐标，保持排序判定准确。
 */
function startDrag(event: MouseEvent, index: number) {
  // 📍 1. 拦截左键，仅响应右键拖拽 (button 值为 2)
  if (event.button !== 2) return;
  if (isDeleteMode.value) { removeSong(index); return; }

  isSorting.value = true;
  dragIndex.value = index;

  // 初始采样各列表项的 Y 轴中点
  staticMidYPoints = songItemsRefs.value.map(el => {
    if (!el) return 0;
    const rect = el.getBoundingClientRect();
    return rect.top + rect.height / 2;
  });

  let swapLock = false; // 📍 2. 新增：拖拽节流冷却锁

  const onMouseMove = (moveEvent: MouseEvent) => {
    // 如果处于冷却期，直接忽略鼠标移动
    if (dragIndex.value === null || swapLock) return;

    let targetGridIndex = 0;
    for (let i = 0; i < staticMidYPoints.length; i++) {
      if (moveEvent.clientY > staticMidYPoints[i]) targetGridIndex = i + 1;
    }
    targetGridIndex = Math.max(0, Math.min(targetGridIndex, staticMidYPoints.length - 1));

    if (targetGridIndex !== dragIndex.value) {
      swapLock = true; // 发生位置交换，立即上锁

      const item = playlist.value.splice(dragIndex.value, 1)[0];
      playlist.value.splice(targetGridIndex, 0, item);
      dragIndex.value = targetGridIndex;

      // 📍 3. 设定 150 毫秒冷却时间，以此降低灵敏度
      setTimeout(() => {
        // DOM 位置已更新，重新采样最新的中点坐标
        staticMidYPoints = songItemsRefs.value.map(el => {
          if (!el) return 0;
          const rect = el.getBoundingClientRect();
          return rect.top + rect.height / 2;
        });
        swapLock = false; // 冷却结束，解锁下一次交换
      }, 150);
    }
  };

  const onMouseUp = async (upEvent: MouseEvent) => {
    // 确保释放的也是右键
    if (upEvent.button !== 2) return;

    isSorting.value = false;
    dragIndex.value = null;
    staticMidYPoints = [];
    window.removeEventListener('mousemove', onMouseMove);
    window.removeEventListener('mouseup', onMouseUp);
    await invoke("sync_playlist", { songs: playlist.value });
  };

  window.addEventListener('mousemove', onMouseMove);
  window.addEventListener('mouseup', onMouseUp);
}

/**
 * 播放指定索引的歌曲。
 *
 * 核心设计：先通过 sync_playlist 将过滤后的列表（filteredSongs）同步到 Rust 后端，
 * 再调用 play_at_index 触发播放。这确保了"所见即所播"——用户在搜索框中筛选后，
 * 播放列表自动变为搜索结果，切换下一首时不会跳回原始的全量列表。
 */
async function playSong(index: number) {
  try {
    // 📍 核心重点：必须同步 filteredSongs.value，做到"所见即所播"
    // 这样搜索过滤后，播放列表也会自动变成这些搜索结果！
    await invoke("sync_playlist", { songs: filteredSongs.value });

    // 然后再告诉后端播放这个界面的索引
    await invoke("play_at_index", { index: index });

  } catch (err) {
    console.error("播放指令发送失败:", err);
    showToast(`❌ 播放失败: ${String(err)}`, "error");
  }
}
async function playNext() { await invoke("play_next"); }
async function playPrev() { await invoke("play_prev"); }
async function togglePlay() {
  if (isPlaying.value) { await invoke("pause_audio"); } else { await invoke("resume_audio"); }
  isPlaying.value = !isPlaying.value;
}

// 📍 新增：用于锁定按钮，防止网络请求期间用户重复点击
const isImporting = ref(false);

// ==========================================
// 歌曲导入智能分发漏斗：
// 通过正则匹配区分三种输入类型——BV号（含多P）、收藏夹（fid）、UP主合集（season_id），
// 分发到对应的 Tauri command。
// 查重逻辑基于 (bvid, cid) 二元组：同一 BV 号的不同分 P 拥有不同的 cid，
// 只有两字段都匹配才判定为重复，确保多 P 视频不会被误过滤。
// ==========================================
// 📍 全局统一的智能分发漏斗
async function addSong() {
  if (isImporting.value) return;
  const inputVal = newBvid.value.trim();
  if (!inputVal) return;

  // 1. 合集 (Season)
  const seasonMatch = inputVal.match(/season_id=(\d+)/i);
  if (seasonMatch) { return await handleBulkImport('import_season_list', { sid: seasonMatch[1] }); }

  // 2. 收藏夹 (Fav)
  const favMatch = inputVal.match(/fid=(\d+)/i) || inputVal.match(/^(\d{5,})$/);
  if (favMatch) { return await handleBulkImport('import_fav_list', { fid: favMatch[1] }); }

  // 3. BV号/分P视频 (Bvid)
  const bvMatch = inputVal.match(/(BV1[A-Za-z0-9]{9})/i);
  const targetBvid = bvMatch ? bvMatch[1] : inputVal;
  return await handleBulkImport('import_bvid', { bvid: targetBvid });
}

// 📍 通用数据处理工厂
async function handleBulkImport(commandName: string, payload: any) {
  isImporting.value = true;
  try {
    const fetchedSongs: any[] = await invoke(commandName, payload);

    // 🚨 核心查重升级：必须 bvid 和 cid 同时一致，才判定为重复！
    const uniqueSongs = fetchedSongs.filter(newSong =>
      !playlist.value.some(existSong =>
        existSong.bvid === newSong.bvid && existSong.cid === newSong.cid
      )
    );

    if (uniqueSongs.length === 0) {
      showToast("⚠️ 注入拦截：本歌单中已经存在该歌曲。", "warning");
    } else {
      playlist.value.push(...uniqueSongs);
      await invoke("sync_playlist", { songs: playlist.value });

      // 如果只导入了一首歌，悄悄切换到下一首逻辑；如果是批量，弹窗提示
      if (uniqueSongs.length > 1) {
        showToast(`✅ 导入完成！过滤重复后新增 ${uniqueSongs.length} 首歌曲。`);
      }
    }
    newBvid.value = "";
  } catch (err) {
    console.error("解析失败:", err);
    showToast(`❌ 注入失败: ${String(err)}`, "error");
  } finally {
    isImporting.value = false;
  }
}

async function toggleMode() {
  const modes = ["ListLoop", "SingleLoop", "Shuffle"];
  currentMode.value = modes[(modes.indexOf(currentMode.value) + 1) % modes.length];
  await invoke("set_playback_mode", { mode: currentMode.value });
}

/**
 * 进度条交互：mousedown 触发拖拽或点击跳转。
 *
 * seekLock 霸体锁机制——跳转后锁定 800ms，在此期间忽略 Rust 后端发来的
 * playback-progress 事件。这是必需的：Seek 请求是异步的，在 Rust 侧完成
 * FFmpeg 重新解码之前，后端可能仍在发送旧位置的进度事件。如果不加锁，
 * 进度条会先跳到目标位置，然后被旧进度事件拉回原位（"回弹"现象），
 * 用户体验极差。800ms 足够 Rust 侧完成 Seek 并开始发送新位置的进度。
 */
function onMouseDown(e: MouseEvent) {
  if (!currentSong.value) return;
  isDragging.value = true;
  // 1. 点下的瞬间，立刻更新一次进度（支持点击跳转）
  calculateProgressFromX(e.clientX);
  // 2. 声明全局移动处理函数
  const handleMouseMove = (moveEvent: MouseEvent) => {
    calculateProgressFromX(moveEvent.clientX);
  };
  // 3. 声明全局鼠标松开处理函数
  const handleMouseUp = async (upEvent: MouseEvent) => {
    // 计算松开时的最终位置
    calculateProgressFromX(upEvent.clientX);
    // 释放状态锁
    seekLock.value = true; // 📍 激活霸体锁
    isDragging.value = false;
    setTimeout(() => {
      seekLock.value = false;
    }, 800);
    // 移除全局监听器，防止内存泄漏
    window.removeEventListener('mousemove', handleMouseMove);
    window.removeEventListener('mouseup', handleMouseUp);
    // 4. 将最终确定的秒数发送给 Rust 后端（假设你的后端的跳转指令叫 seek）
    try {
      await invoke('seek_audio', { secs: Math.floor(currentProgress.value) });
    } catch (err) {
      console.error("发送跳转指令失败:", err);
    }
  };
  // 5. 将事件注册到 window 上，确保鼠标移出进度条范围仍能丝滑拖拽
  window.addEventListener('mousemove', handleMouseMove);
  window.addEventListener('mouseup', handleMouseUp);
}
const formatTime = (secs: number) => {
  if (isNaN(secs)) return "0:00";
  const s = Math.floor(secs); // 先把秒数强制变成整数
  return `${Math.floor(s / 60)}:${(s % 60).toString().padStart(2, '0')}`;
};

const showExitModal = ref(false);

// 📍 1. 所有的系统级监听都统一塞进 onMounted 里
onMounted(async () => {
  await initDatabase();
  await loadAudioDevices();

  // 📍 3. 软件启动时，立刻将读取到的音量下发给后端
  await invoke("set_volume", { vol: getPhysicalVolume(Number(currentVolume.value)) }).catch(e => console.error(e));

  if (allPlaylists.value['default']) {
    await invoke("sync_playlist", { songs: allPlaylists.value['default'].songs });
  }

  await listen('playback-start', (event) => {
    currentSong.value = event.payload as Song;
    currentProgress.value = 0;
    isPlaying.value = true;

  });

  if (allPlaylists.value['default']) {
    await invoke("sync_playlist", { songs: allPlaylists.value['default'].songs });
  }

  await listen('playback-start', (event) => {
    currentSong.value = event.payload as Song;
    currentProgress.value = 0;
    isPlaying.value = true;
  });

  await listen('toggle-exit-modal', () => {
  console.log("✅ 前端已捕获信号！");
  showExitModal.value = true;
  });

  // 📍 修复：把进度监听搬回 onMounted 内部！
  await listen('playback-progress', (event: any) => {
    // 如果用户正在拖拽进度条，或者处于霸体状态，直接忽略 Rust 发来的时间更新
    if (isDragging.value || seekLock.value) return;
    currentProgress.value = event.payload;
  });
});

// 📍 2. 这两个是普通的函数，放在最外层是没问题的，去掉前面的缩进即可
async function handleHideToTray() {
  showExitModal.value = false;
  await invoke("hide_window"); // 藏到系统托盘（音频引擎继续跑）
}

async function handleForceExit() {
  await invoke("force_exit"); // 直接断电
}

// ==========================================
// ⚙️ 系统设置与数据管理：
// 导出使用 Tauri 原生保存对话框（tauri-plugin-dialog），
// 由 Rust 后端执行实际文件写入（save_file command）。
// 导入时校验 JSON 结构必须包含 "default" 键，防止加载损坏的备份文件。
// ==========================================
const showSettingsModal = ref(false);
const showResetConfirmModal = ref(false); // 📍 新增：二次确认弹窗状态
const importFileRef = ref<HTMLInputElement | null>(null);

function toggleSettings() {
  showSettingsModal.value = !showSettingsModal.value;
}

// 📍 升级版：调用原生另存为窗口并让 Rust 写入
async function exportDatabase() {
  try {
    const dataStr = JSON.stringify(allPlaylists.value, null, 2);

    // 呼出系统原生保存弹窗
    const filePath = await save({
      title: "导出播放器备份",
      defaultPath: `bvplayer_backup_${new Date().toISOString().split('T')[0]}.json`,
      filters: [{ name: 'JSON 数据', extensions: ['json'] }]
    });

    // 如果用户点了"保存"而不是"取消"
    if (filePath) {
      // 呼叫我们刚才写的 Rust 函数
      await invoke('save_file', { path: filePath, contents: dataStr });
      showToast("✅ 数据备份已成功导出！");
    }
  } catch (error) {
    console.error("导出失败:", error);
    showToast("❌ 导出失败，请检查控制台。");
  }
}

function triggerImport() {
  if (importFileRef.value) {
    importFileRef.value.click();
  }
}

function onImportFileSelected(event: Event) {
  const input = event.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) return;

  const file = input.files[0];
  const reader = new FileReader();

  reader.onload = (e) => {
    try {
      const result = e.target?.result as string;
      const parsedData = JSON.parse(result);

      if (!parsedData || typeof parsedData !== 'object' || !parsedData['default']) {
        throw new Error("无效的数据格式");
      }

      allPlaylists.value = parsedData;
      saveDatabase();
      showToast("✅ 数据恢复成功！");
      showSettingsModal.value = false;
    } catch (error) {
      console.error("导入失败:", error);
      showToast("❌ 导入失败：文件格式错误或已损坏。");
    } finally {
      input.value = "";
    }
  };
  reader.readAsText(file);
}

// 📍 改为仅唤出二次确认弹窗
function triggerClearAllData() {
  showResetConfirmModal.value = true;
}

// 📍 确认清空的最终执行函数
function executeClearAllData() {
  localStorage.removeItem('b-player-database');
  allPlaylists.value = {
    "default": {
      id: "default",
      name: "默认歌单",
      cover: "https://i0.hdslb.com/bfs/archive/478a05f0134440062b1aab93d62957bda52b57e9.jpg",
      songs: []
    }
  };
  saveDatabase();
  showToast("🗑️ 数据已彻底清空。");
  showResetConfirmModal.value = false;
  showSettingsModal.value = false;
}
</script>

<template>
  <Transition name="toast-fade">
  <div v-if="toast.visible" class="custom-toast" :class="toast.type">
    {{ toast.message }}
  </div>
</Transition>
  <div v-if="showExitModal" class="modal-overlay">
    <div class="cyber-modal">
      <div class="modal-header">
        <h3>⚠️ 关闭确认</h3>
        <button class="close-btn" @click="showExitModal = false">✖</button>
      </div>
      <div class="modal-body">
        <p>选择 <strong class="highlight-tray">[挂起至托盘]</strong> 将最小化为托盘。</p>
        <p>选择 <strong class="highlight-exit">[彻底退出]</strong> 将程序强制退出。</p>
      </div>
      <div class="modal-actions">
        <button class="btn-tray" @click="handleHideToTray">挂起至托盘</button>
        <button class="btn-exit" @click="handleForceExit">彻底退出</button>
      </div>
    </div>
  </div>

  <div v-if="showSettingsModal" class="modal-overlay" @click.self="showSettingsModal = false">
    <div class="cyber-modal settings-modal">
      <div class="modal-header">
        <h3>⚙️ 系统设置</h3>
        <button class="close-btn" @click="showSettingsModal = false">✖</button>
      </div>
      <div class="modal-body">
        <div class="settings-group">
          <h4>💾 数据管理</h4>
          <p class="settings-desc">将你的本地歌单导出为 JSON 文件，或从备份文件中恢复。</p>
          <div class="settings-actions">
            <button class="btn-secondary" @click="exportDatabase">📤 导出备份</button>
            <button class="btn-secondary" @click="triggerImport">📥 导入恢复</button>
          </div>
        </div>

        <div class="settings-group danger-zone">
          <h4>⚠️ 危险区域</h4>
          <p class="settings-desc">清空所有本地数据，恢复至初始状态。</p>
          <button class="btn-exit" @click="triggerClearAllData">🗑️ 恢复出厂设置</button>
        </div>
      </div>
    </div>
  </div>

  <div v-if="showResetConfirmModal" class="modal-overlay" style="z-index: 9999999;">
    <div class="cyber-modal">
      <div class="modal-header">
        <h3>⚠️ 最终确认</h3>
        <button class="close-btn" @click="showResetConfirmModal = false">✖</button>
      </div>
      <div class="modal-body">
        <p style="color: #f38ba8; font-weight: bold; font-size: 1.05rem;">正在进行危险操作！</p>
        <p>将永久清空所有歌单数据和本地缓存。该操作<strong style="text-decoration: underline;">不可逆</strong>！</p>
        <p>是否继续？</p>
      </div>
      <div class="modal-actions">
        <button class="btn-secondary" @click="showResetConfirmModal = false">取消，手滑了</button>
        <button class="btn-exit" style="background: #f38ba8; color: #11111b; font-weight: bold;" @click="executeClearAllData">确认彻底清空</button>
      </div>
    </div>
  </div>

  <div class="window-safe-area">
    <div class="app-layout" @contextmenu.prevent>

      <input type="file" ref="importFileRef" accept=".json" style="display: none;" @change="onImportFileSelected" />
      <input type="file" ref="coverInputRef" accept="image/png, image/jpeg, image/webp" style="display: none;" @change="onCoverFileSelected" />

      <div class="main-viewport">

        <div v-if="currentView === 'library'" class="library-view">
          <header class="lib-header">
            <h1>🗃️ 歌单列表 </h1>
            <div class="create-box">
              <div class="expandable-search" :class="{ 'is-active': isLibSearchActive || searchPlaylistQuery }">
                <button class="btn-icon" @click="activateLibSearch" title="搜索歌单">🔍</button>
                <input
                  ref="libSearchInputRef"
                  type="text"
                  v-model="searchPlaylistQuery"
                  placeholder="搜索歌单..."
                  class="search-input-hidden"
                  @blur="onLibSearchBlur"
                />
              </div>

              <div class="expandable-search" :class="{ 'is-active': isCreateActive || newPlaylistName }">
                <button class="btn-icon" @click="activateCreate" title="新建歌单 (点击或回车确认)">➕</button>
                <input
                  ref="createInputRef"
                  type="text"
                  v-model="newPlaylistName"
                  placeholder="输入新歌单名称..."
                  class="search-input-hidden"
                  @blur="onCreateBlur"
                  @keyup.enter="createNewPlaylist(); isCreateActive = false"
                />
              </div>

              <button @click="toggleSettings" class="btn-icon" title="系统设置">⚙️</button>
            </div>
          </header>

          <div class="playlist-grid">
            <div v-for="pl in filteredPlaylists" :key="pl.id" class="pl-card" @click="enterPlaylist(pl.id)">
              <div class="pl-cover-wrapper">
                <img :src="pl.cover" referrerpolicy="no-referrer" class="pl-cover" />
                <div class="pl-actions">
                  <button @click="changePlaylistCover(pl.id, $event)" class="btn-small" title="本地选择封面">🖼️</button>
                  <button v-if="pl.id !== 'default'" @click="deletePlaylist(pl.id, $event)" class="btn-small danger" title="删除歌单">🗑️</button>
                </div>
              </div>
              <div class="pl-info">
                <div class="pl-name">{{ pl.name }}</div>
                <div class="pl-count">{{ pl.songs.length }} 首母带</div>
              </div>
            </div>
          </div>
        </div>

        <div v-else class="queue-panel">
          <header class="detail-header">
            <button @click="currentView = 'library'" class="btn-back">⬅ 返回</button>
            <h2>{{ allPlaylists[activePlaylistId]?.name }} ({{ playlist.length }})</h2>

            <div class="input-group">
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

              <div class="expandable-search" :class="{ 'is-active': isAddActive || newBvid }">
                <button class="btn-icon" @click="activateAdd" title="添加歌曲 (点击或回车确认)">
                  {{ isImporting ? '⏳' : '➕' }}
                </button>
                <input
                  ref="addInputRef"
                  type="text"
                  v-model="newBvid"
                  placeholder="输入 BV/链接 (回车)"
                  class="search-input-hidden"
                  @blur="onAddBlur"
                  @keyup.enter="addSong(); isAddActive = false"
                  :disabled="isImporting"
                />
              </div>

              <button
                @click="isDeleteMode = !isDeleteMode"
                class="btn-icon danger-toggle"
                :class="{ 'active': isDeleteMode }"
                :title="isDeleteMode ? '退出删除模式' : '开启删除模式'"
              >
                {{ isDeleteMode ? '🛑' : '🗑️' }}
              </button>
            </div>
          </header>
          <ul v-if="playlist.length > 0" :class="['song-list', { 'delete-mode': isDeleteMode }]">
            <li v-for="(song, index) in filteredSongs" :key="song.bvid"
              :ref="el => { if (el) songItemsRefs[index] = el as HTMLElement }"
              :class="['song-item', { 'is-active': currentSong?.bvid === song.bvid, 'is-dragging': isSorting && dragIndex === index }]"
              @mousedown="startDrag($event, index)" @dblclick="playSong(index)">
              <img :src="song.cover_url" referrerpolicy="no-referrer" class="cover-art" />
              <div class="info">
                <div class="title">{{ song.title }}</div>
                <div class="author">{{ song.author }}</div>
              </div>
              <div class="duration">{{ formatTime(song.duration) }}</div>
            </li>
          </ul>
          <div v-else class="empty-state">歌单空空如也，请在上方填入视频链接或BV号</div>
        </div>
      </div>

      <footer class="player-console">
        <div class="progress-bar-bg" v-if="currentSong" ref="progressBarRef" @mousedown="onMouseDown">
          <div class="progress-fill" :class="{ 'is-dragging': isDragging }"
            :style="{ width: (currentProgress / currentSong.duration) * 100 + '%' }"></div>
        </div>
        <div class="console-inner">
          <div class="now-playing" v-if="currentSong">
            <img :src="currentSong.cover_url" referrerpolicy="no-referrer" class="np-cover" />
            <div class="np-info">
              <div class="np-title">{{ currentSong.title }}</div>
              <div class="np-time">{{ formatTime(currentProgress) }} / {{ formatTime(currentSong.duration) }}</div>
            </div>
          </div>
          <div class="now-playing empty" v-else>⚡ 音频待命...</div>

          <div class="control-hub">
            <select v-model="selectedDevice" @change="changeDevice" class="device-selector" title="输出硬件">
              <option value="">💻 系统默认输出</option>
              <option v-for="dev in audioDevices" :key="dev" :value="dev">{{ dev }}</option>
            </select>

            <div class="control-divider"></div>

            <div class="bili-volume-wrap">
              <div class="volume-icon" @click="toggleMute" title="静音 (m)">
                {{ isMuted ? '🔇' : (currentVolume > 0.5 ? '🔊' : (currentVolume > 0 ? '🔉' : '🔇')) }}
              </div>
              <div class="volume-slider-drawer">
                <input type="range" min="0" max="1" step="0.01" v-model.number="currentVolume" @input="updateVolume"
                  class="bili-slider"
                  :style="{ background: `linear-gradient(to right, #89b4fa ${currentVolume * 100}%, rgba(255, 255, 255, 0.1) ${currentVolume * 100}%)` }"
                />
              </div>
            </div>

            <div class="control-divider"></div>

            <button @click="toggleMode" class="btn-icon mode-btn" :title="modeTitles[currentMode]">
              {{ modeIcons[currentMode] }}
            </button>

            <button @click="playPrev" class="btn-icon" title="上一首">⏮️</button>

            <button @click="togglePlay" class="btn-play" :disabled="!currentSong" :title="isPlaying ? '暂停' : '播放'">
              {{ isPlaying ? '⏸️' : '▶️' }}
            </button>

            <button @click="playNext" class="btn-icon" title="下一首">⏭️</button>
          </div>
        </div>
      </footer>

    </div>
  </div>
</template>

<style scoped>
/* =======================================
   1. 基础布局 (响应式全屏铺开)
======================================= */
.window-safe-area {
  width: 100vw;
  height: 100vh;
  /* 删除了之前的居中对齐，直接作为最外层全屏容器 */
  background-color: #11111b; /* 统一使用你的主色调 */
  overflow: hidden;
}

.app-layout {
  display: flex;
  flex-direction: column;

  /* 📍 核心修改：删除了 aspect-ratio 和 max-width/height 限制 */
  width: 100%;
  height: 100%;

  background-color: #11111b;
  overflow: hidden;
}

.main-viewport {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  scroll-behavior: smooth;
}

/* 通用按钮质感与按压动效 */
button {
  transition: all 0.2s cubic-bezier(0.25, 0.8, 0.25, 1);
  outline: none;
}

button:active:not(:disabled) {
  transform: scale(0.95);
}

/* 物理按压反馈 */

.btn-primary {
  background: linear-gradient(135deg, #89b4fa, #b4befe);
  color: #11111b;
  border: none;
  padding: 8px 18px;
  cursor: pointer;
  font-weight: 600;
  border-radius: 6px;
  box-shadow: 0 4px 10px rgba(137, 180, 250, 0.2);
}

.btn-primary:hover {
  box-shadow: 0 6px 15px rgba(137, 180, 250, 0.4);
  transform: translateY(-1px);
}

input {
  background: rgba(24, 24, 37, 0.8);
  border: 1px solid #313244;
  color: #cdd6f4;
  padding: 10px 14px;
  outline: none;
  border-radius: 6px;
  transition: border-color 0.2s;
  font-size: 0.9rem;
}

input:focus {
  border-color: #89b4fa;
  background: #181825;
}

/* =======================================
   2. 媒体库大厅视图 (Library)
======================================= */
.library-view {
  padding: 35px 40px;
}

.lib-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 35px;
}

.lib-header h1 {
  font-size: 1.6rem;
  color: #cdd6f4;
  font-weight: 700;
  margin: 0;
  letter-spacing: 0.5px;
}

.create-box {
  display: flex;
  gap: 12px;
}

.playlist-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: 25px;
}

/* 歌单卡片：微光边框与弥散阴影 */
.pl-card {
  background: #181825;
  border-radius: 12px;
  overflow: hidden;
  cursor: pointer;
  transition: all 0.3s ease;
  border: 1px solid rgba(255, 255, 255, 0.03);
  box-shadow: 0 8px 20px rgba(0, 0, 0, 0.15);
}

.pl-card:hover {
  transform: translateY(-6px);
  border-color: rgba(137, 180, 250, 0.4);
  box-shadow: 0 12px 25px rgba(0, 0, 0, 0.3);
}

.pl-cover-wrapper {
  position: relative;
  width: 100%;
  aspect-ratio: 1;
  background: #1e1e2e;
  overflow: hidden;
}

.pl-cover {
  width: 100%;
  height: 100%;
  object-fit: cover;
  transition: transform 0.5s ease, filter 0.3s ease;
}

.pl-card:hover .pl-cover {
  transform: scale(1.05);
  filter: brightness(0.7);
}

/* 悬浮时图片轻微放大变暗 */

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

.pl-card:hover .pl-actions {
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

.pl-info {
  padding: 18px;
}

.pl-name {
  color: #cdd6f4;
  font-weight: 600;
  font-size: 1.05rem;
  margin-bottom: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.pl-count {
  color: #6c7086;
  font-size: 0.85rem;
  font-weight: 500;
}

/* =======================================
   3. 播放列表详情视图 (Queue)
======================================= */
.queue-panel {
  padding: 25px 40px;
  display: flex;
  flex-direction: column;
  height: 100%;
}

.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 25px;
  padding-bottom: 25px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
}

.btn-back {
  background: transparent;
  border: 1px solid #45475a;
  color: #a6adc8;
  padding: 8px 16px;
  border-radius: 6px;
  font-weight: 500;
}

.btn-back:hover {
  background: #313244;
  color: #cdd6f4;
  border-color: #585b70;
}

.detail-header h2 {
  color: #cdd6f4;
  margin: 0 0 0 20px;
  font-size: 1.4rem;
  flex: 1;
  text-align: left;
  font-weight: 700;
}

.input-group {
  display: flex;
  gap: 12px;
}


.song-list {
  list-style: none;
  display: flex;
  flex-direction: column;
  gap: 6px;
  padding: 0;
  margin: 0;
  padding-bottom: 20px;
}

.song-item {
  display: flex;
  align-items: center;
  gap: 18px;
  background: transparent;
  padding: 10px 15px;
  border-radius: 8px;
  border-left: 3px solid transparent;
  transition: all 0.2s ease;
  /* 📍 删除原有的 cursor: grab; */
}

.song-item:active {
  /* 📍 删除原有的 cursor: grabbing; */
  background: rgba(255, 255, 255, 0.05);
}

.song-item.is-dragging {
  opacity: 0.5;
  background: #181825;
  transform: scale(0.98);
  box-shadow: 0 5px 15px rgba(0, 0, 0, 0.3);
  cursor: grabbing; /* 📍 仅在触发右键拖拽时，显示抓取图标 */
}

/* 📍 新增：明确允许标题和作者字段的文本被鼠标左键选中 */
.title, .author {
  user-select: text;
}

.song-list.delete-mode .song-item {
  cursor: crosshair;
}

.song-list.delete-mode .song-item:hover {
  background: rgba(243, 139, 168, 0.1);
  border-left-color: #f38ba8;
}

.cover-art {
  width: 48px;
  height: 48px;
  border-radius: 6px;
  object-fit: cover;
  pointer-events: none;
  box-shadow: 0 2px 5px rgba(0, 0, 0, 0.2);
}

.info {
  flex: 1;
  pointer-events: none;
  display: flex;
  flex-direction: column;
  justify-content: center;
}

.title {
  font-weight: 600;
  color: #cdd6f4;
  font-size: 0.95rem;
  margin-bottom: 4px;
}

.author {
  font-size: 0.8rem;
  color: #7f849c;
}

.duration {
  color: #a6adc8;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.85rem;
  background: rgba(255, 255, 255, 0.05);
  padding: 4px 8px;
  border-radius: 4px;
}

.empty-state {
  text-align: center;
  color: #585b70;
  margin-top: 60px;
  font-size: 1.1rem;
  font-weight: 500;
}

/* =======================================
   4. 底部播放控制台 (Player Console) - 毛玻璃特效
======================================= */
.player-console {
  background: rgba(24, 24, 37, 0.85);
  backdrop-filter: blur(12px);
  -webkit-backdrop-filter: blur(12px);
  border-top: 1px solid rgba(255, 255, 255, 0.05);
  position: relative;
  z-index: 10;
  flex-shrink: 0;
}

/* 进度条发光效果 */
.progress-bar-bg {
  width: 100%;
  height: 8px;
  /* 默认直接加粗到 8px，提供充足的点击面积 */
  background: rgba(255, 255, 255, 0.08);
  /* 稍微提亮一点底色 */
  position: absolute;
  top: -8px;
  /* 贴合控制台顶部边缘 */
  cursor: pointer;
  transition: background 0.2s;
  /* 去掉高度变化，只保留背景色变化的过渡 */
}

.progress-bar-bg:hover {
  background: rgba(255, 255, 255, 0.15);
  /* 悬浮时仅提亮底板，作为交互反馈 */
}

/* 📍 进度条填充 - 丝滑走带版 */
.progress-fill {
  height: 100%;
  background: #a6e3a1;
  /* 核心平滑魔法：将原本的 0.1s 改长。
     如果你的 Rust 后端是每 1 秒发送一次进度更新，这里就写 1s linear。
     浏览器会自动在这 1 秒内进行补间动画，让进度条如同丝绸般平滑滑动。
     （如果你后端是 500ms 发一次，这里就改为 0.5s linear）
  */
  transition: width 0.5s linear;
  box-shadow: 0 0 12px rgba(166, 227, 161, 0.7);
  /* 发光特效微调得更饱满 */
  border-radius: 0 4px 4px 0;
}

/* 拖拽时的极速响应锁 */
.progress-fill.is-dragging {
  /* 拖拽时必须强行打断 transition 动画，确保进度条能瞬间跟手，没有延迟滞后感 */
  transition: none !important;
}

.console-inner {
  padding: 15px 30px;
  display: flex;
  justify-content: space-between;
  align-items: center;
  height: 85px;
}

.now-playing {
  display: flex;
  align-items: center;
  gap: 18px;
  width: 35%;
}

.now-playing.empty {
  color: #585b70;
  font-family: monospace;
  font-size: 0.9rem;
}

.np-cover {
  width: 56px;
  height: 56px;
  border-radius: 8px;
  object-fit: cover;
  box-shadow: 0 4px 8px rgba(0, 0, 0, 0.3);
}

.np-title {
  font-weight: 700;
  color: #cdd6f4;
  font-size: 1.05rem;
  margin-bottom: 4px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  max-width: 250px;
}

.np-time {
  color: #7f849c;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.85rem;
}

.control-hub {
  display: flex;
  gap: 15px;
  align-items: center;
}

.btn-icon {
  background: transparent;
  border: none;
  color: #a6adc8;
  padding: 10px;
  border-radius: 8px;
  font-size: 1.1rem;
}

.btn-icon:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #cdd6f4;
}

/* =======================================
   硬件设备下拉框 (暗黑胶囊风格)
======================================= */
.device-selector {
  background: transparent;
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #a6adc8;
  padding: 6px 14px;
  border-radius: 20px; /* 改为圆润的胶囊形 */
  font-size: 0.85rem;
  outline: none;
  max-width: 140px;
  cursor: pointer;
  transition: all 0.2s;
  appearance: none;
  text-align: center;
}

.device-selector:hover {
  background: rgba(255, 255, 255, 0.05);
  color: #cdd6f4;
  border-color: rgba(255, 255, 255, 0.2);
}

/* 视觉分割线 */
.control-divider {
  width: 1px;
  height: 18px;
  background: rgba(255, 255, 255, 0.1);
  margin: 0 4px;
}

/* =======================================
   核心播放按钮 (悬浮发光圆环)
======================================= */
.btn-play {
  background: #cdd6f4;
  border: none;
  color: #11111b;
  width: 42px;
  height: 42px;
  border-radius: 50%; /* 绝对的圆形 */
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.2rem;
  cursor: pointer;
  box-shadow: 0 4px 12px rgba(205, 214, 244, 0.2);
  transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  margin: 0 6px;
}

.btn-play:hover:not(:disabled) {
  background: #ffffff;
  transform: scale(1.08); /* 悬浮时轻微放大 */
  box-shadow: 0 6px 18px rgba(205, 214, 244, 0.4);
}

.btn-play:active:not(:disabled) {
  transform: scale(0.95);
}

.btn-play:disabled {
  background: #313244;
  color: #585b70;
  cursor: not-allowed;
  box-shadow: none;
}

.device-selector {
  background: rgba(17, 17, 27, 0.6);
  border: 1px solid #313244;
  color: #a6adc8;
  padding: 8px 12px;
  border-radius: 6px;
  font-size: 0.85rem;
  outline: none;
  max-width: 150px;
  cursor: pointer;
  transition: all 0.2s;
  appearance: none;
}

.device-selector:hover {
  border-color: #89b4fa;
  color: #cdd6f4;
  background: rgba(17, 17, 27, 0.9);
}

/* ==========================================
   仿 B 站网页版抽屉式音量交互
   ========================================== */
.bili-volume-wrap {
  display: flex;
  align-items: center;
  height: 100%;
  padding: 0 5px;
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

.volume-icon:hover {
  color: #00aeec;
  /* B站主题蓝 */
}

/* 抽屉容器：默认宽度为 0 且透明，隐藏滑块 */
.volume-slider-drawer {
  width: 0;
  opacity: 0;
  overflow: hidden;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  /* B站常用的丝滑阻尼曲线 */
  display: flex;
  align-items: center;
}

/* 核心魔法：鼠标悬浮整个包裹区时，展开滑块 */
.bili-volume-wrap:hover .volume-slider-drawer {
  width: 70px;
  opacity: 1;
  margin-left: 5px;
}

/* 📍 轨道本体：纯净版 */
.bili-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 6px; /* 舒适的进度条厚度 */
  border-radius: 3px;

  /* 魔法隔离：强行干掉全局 input 带来的外框和内边距 */
  border: none !important;
  padding: 0 !important;
  outline: none !important;

  cursor: pointer;
  /* 背景由 Vue 的内联 style 接管计算，做到绝对跟手 */
}

/* 📍 彻底隐藏拖拽点 (Thumb)，但保留原生的拖拽物理判定 */
.bili-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 1px; /* 极窄宽度，确保点击计算极致精准 */
  height: 12px;
  background: transparent; /* 隐身术 */
  border: none;
  box-shadow: none;
}

/* =======================================
   5. 退出确认模态框 (Cyber Modal)
======================================= */
.modal-overlay {
  /* 📍 核心修复：改为 fixed，直接相对于视口固定，无视任何父容器定位 */
  position: fixed !important;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;

  /* 确保 z-index 足够高 */
  z-index: 999999 !important;

  background: rgba(17, 17, 27, 0.7);
  backdrop-filter: blur(8px);
  -webkit-backdrop-filter: blur(8px);

  /* 确保 flex 布局正常工作 */
  display: flex !important;
  align-items: center;
  justify-content: center;
}

.cyber-modal {
  background: #181825;
  border: 1px solid #313244;
  border-radius: 12px;
  width: 420px;
  box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.05) inset;
  animation: modal-pop 0.3s cubic-bezier(0.175, 0.885, 0.32, 1.275);
}

.modal-header {
  padding: 16px 24px;
  border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  display: flex;
  justify-content: space-between;
  align-items: center;
}

.modal-header h3 {
  margin: 0;
  font-size: 1.15rem;
  color: #f9e2af;
  font-weight: 700;
  letter-spacing: 1px;
}

.close-btn {
  background: transparent;
  border: none;
  color: #a6adc8;
  cursor: pointer;
  font-size: 1.2rem;
  padding: 4px;
  transition: all 0.2s;
}

.close-btn:hover {
  color: #f38ba8;
  transform: rotate(90deg);
}

.modal-body {
  padding: 30px 24px;
  color: #cdd6f4;
  font-size: 0.95rem;
  line-height: 1.8;
}

.highlight-tray {
  color: #89b4fa;
  font-weight: 600;
}

.highlight-exit {
  color: #f38ba8;
  font-weight: 600;
}

.modal-actions {
  padding: 16px 24px;
  display: flex;
  justify-content: flex-end;
  gap: 16px;
  background: rgba(17, 17, 27, 0.5);
  border-radius: 0 0 12px 12px;
}

.btn-tray,
.btn-exit {
  padding: 10px 20px;
  border-radius: 6px;
  cursor: pointer;
  font-weight: 700;
  border: none;
  transition: all 0.2s;
}

.btn-tray {
  background: #89b4fa;
  color: #11111b;
}

.btn-tray:hover {
  background: #b4befe;
  transform: translateY(-1px);
}

.btn-exit {
  background: transparent;
  border: 1px solid #f38ba8;
  color: #f38ba8;
}

.btn-exit:hover {
  background: rgba(243, 139, 168, 0.15);
  box-shadow: 0 0 10px rgba(243, 139, 168, 0.3);
}

@keyframes modal-pop {
  from {
    transform: scale(0.9) translateY(10px);
    opacity: 0;
  }

  to {
    transform: scale(1) translateY(0);
    opacity: 1;
  }
}
/* =======================================
   6. 设置面板特定样式
======================================= */
.settings-modal {
  width: 480px; /* 设置面板稍微宽一点 */
}

.settings-group {
  margin-bottom: 24px;
  padding-bottom: 16px;
  border-bottom: 1px dashed rgba(255, 255, 255, 0.05);
}

.settings-group:last-child {
  border-bottom: none;
  margin-bottom: 0;
  padding-bottom: 0;
}

.settings-group h4 {
  margin: 0 0 8px 0;
  color: #cdd6f4;
  font-size: 1rem;
}

.settings-desc {
  color: #7f849c;
  font-size: 0.85rem;
  margin-top: 0;
  margin-bottom: 16px;
}

.settings-actions {
  display: flex;
  gap: 12px;
}

.btn-secondary {
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #cdd6f4;
  padding: 8px 16px;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.2s;
}

.btn-secondary:hover {
  background: rgba(255, 255, 255, 0.1);
  border-color: #89b4fa;
  color: #89b4fa;
}

.danger-zone h4 {
  color: #f38ba8;
}

.btn-exit:hover {
  background: #f38ba8 !important;
  color: #11111b !important;
}
/* ==========================================
   仿 B 站网页版抽屉式音量交互 (已优化)
   ========================================== */
.bili-volume-wrap {
  display: flex;
  align-items: center;
  height: 100%;
  /* 📍 扩大隐形交互面积，防止鼠标稍微上下偏移就导致抽屉收回 */
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

.volume-icon:hover {
  color: #89b4fa; /* 统一使用主题天际蓝 */
}

/* 抽屉容器 */
.volume-slider-drawer {
  width: 0;
  opacity: 0;
  overflow: hidden;
  /* 📍 核心交互优化：收起时增加 0.25s 延迟，给予用户手抖容错时间 */
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              opacity 0.3s cubic-bezier(0.4, 0, 0.2, 1),
              margin 0.3s cubic-bezier(0.4, 0, 0.2, 1);
  transition-delay: 0.25s;
  display: flex;
  align-items: center;
}

/* 鼠标悬浮包裹区时，展开滑块 */
.bili-volume-wrap:hover .volume-slider-drawer {
  width: 80px; /* 稍微加长，便于精细调节 */
  opacity: 1;
  margin-left: 8px;
  /* 📍 展开时无延迟，立即响应 */
  transition-delay: 0s;
}

/* 轨道本体 */
.bili-slider {
  -webkit-appearance: none;
  appearance: none;
  width: 100%;
  height: 4px; /* 稍微加粗，提升点击命中率 */
  border-radius: 2px;
  outline: none;
  cursor: pointer;
  /* 背景颜色由 Vue 内联 style 动态接管计算 */
}

/* 拖拽点 (Thumb) */
.bili-slider::-webkit-slider-thumb {
  -webkit-appearance: none;
  appearance: none;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  background: #89b4fa;
  /* 📍 增加柔和的发光阴影 */
  box-shadow: 0 0 8px rgba(137, 180, 250, 0.6);
  /* 📍 使用更具弹性的贝塞尔曲线 */
  transition: transform 0.2s cubic-bezier(0.175, 0.885, 0.32, 1.275), box-shadow 0.2s;
}

.bili-slider::-webkit-slider-thumb:hover {
  transform: scale(1.35);
  box-shadow: 0 0 12px rgba(137, 180, 250, 0.9);
}
/* ==========================================
   📍 Toast 轻提示 UI 样式 (暗黑低饱和度适配版)
   ========================================== */
.custom-toast {
  position: fixed;
  top: 24px;
  left: 50%;
  transform: translateX(-50%);
  z-index: 9999;
  padding: 12px 24px;
  border-radius: 6px;
  font-size: 14px;
  font-weight: 400; /* 降低字重，减少突兀感 */
  color: #e0e0e0; /* 使用柔和的灰白色代替纯白 */
  pointer-events: none;
  box-shadow: 0 4px 12px rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  gap: 8px;

  /* 统一使用深色透明底板，贴合播放器整体的暗色调 */
  background: rgba(30, 30, 36, 0.9);
  border: 1px solid rgba(255, 255, 255, 0.08);
}

/* 采用左侧边框指示状态，避免大面积高饱和色块 */
.custom-toast.success {
  border-left: 3px solid #5c8a60; /* 低饱和灰绿 */
}

.custom-toast.warning {
  border-left: 3px solid #bda057; /* 低饱和灰黄 */
}

.custom-toast.error {
  border-left: 3px solid #a35656; /* 低饱和砖红 */
}

/* 📍 Vue Transition 进出场动画 */
.toast-fade-enter-active,
.toast-fade-leave-active {
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
}

.toast-fade-enter-from,
.toast-fade-leave-to {
  opacity: 0;
  transform: translate(-50%, -15px); /* 缩短滑入距离，显得更干脆 */
}

/* =======================================
   🔍 折叠式搜索框动画 (Expandable Search)
======================================= */
.expandable-search {
  display: flex;
  align-items: center;
  background: transparent;
  border-radius: 6px;
  border: 1px solid transparent;
  transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1); /* 顺滑的贝塞尔曲线 */
  overflow: hidden;
  height: 38px; /* 强制高度与旁边 input 对齐 */
}

/* 激活状态：背景浮现，边框亮起 */
.expandable-search.is-active {
  background-color: rgba(30, 30, 36, 0.6);
  border-color: rgba(255, 255, 255, 0.1);
}

.expandable-search .btn-icon {
  margin: 0;
  padding: 8px;
  height: 100%;
}

/* 隐藏状态下的输入框 */
.search-input-hidden {
  width: 0;
  padding: 0;
  border: none;
  background: transparent;
  color: #e0e0e0;
  outline: none;
  font-size: 0.9rem;
  transition: width 0.3s cubic-bezier(0.4, 0, 0.2, 1), padding 0.3s ease;
}

/* 激活状态下的输入框：展开宽度 */
.expandable-search.is-active .search-input-hidden {
  width: 160px; /* 展开后的宽度，可自行调节 */
  padding: 0 12px 0 4px;
}

.search-input-hidden::placeholder {
  color: rgba(255, 255, 255, 0.3);
}

/* =======================================
   🗑️ 删除模式开关动画
======================================= */
.danger-toggle {
  transition: all 0.25s cubic-bezier(0.4, 0, 0.2, 1);
  background: transparent;
}

/* 鼠标悬浮时的警告提示 */
.danger-toggle:hover {
  color: #f38ba8;
  background: rgba(243, 139, 168, 0.1);
}

/* 激活删除模式时的战术高亮 */
.danger-toggle.active {
  color: #11111b; /* 变为深色反差图标 */
  background: #f38ba8;
  box-shadow: 0 0 12px rgba(243, 139, 168, 0.5);
  transform: scale(1.05);
}
</style>