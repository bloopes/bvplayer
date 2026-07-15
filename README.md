- # BVPlayer (V1.0 稳定版)

  一款极简、低占用、无弹窗的本地 Bilibili 桌面音频播放器。

  本项目在 V1.0 阶段采用 **C/S 守护进程架构** (Client/Server Daemon Architecture)，将 UI 渲染与底层音频引擎彻底解耦。即使关闭主界面，音频守护线程依然可以在系统托盘后台静默运行。

  ---

  ## ✨ 核心特性

  *   **JIT 即时解码与防盗链拉取**
      *   绕过常规网页限制，通过 B 站 API 即时拉取底层音频流 (DASH/M4S)。
      *   调用本地 FFmpeg 子进程，通过 PCM 管道 (Pipe) 实现音频流的边下边播与本地持久化缓存。
  *   **双键精准查重与合集解析**
      *   针对 B 站多 P 视频与 UP 主合集，独创 `bvid` + `cid` 联合主键校验机制。
      *   完美解析同一 BV 号下的不同音轨，防止同名切片被错误拦截。
  *   **无缝热插拔与音频路由**
      *   基于 `cpal` 实时枚举操作系统音频物理硬件。
      *   支持在播放过程中无缝切换输出设备（如扬声器、耳机或 VB-Cable 虚拟音频线缆），切换时自动继承播放进度与音量状态。
  *   **硬件级防爆音与指数音量控制**
      *   在音频物理管道 (`rodio::Sink`) 建立的毫秒级瞬间，强制注入状态机历史音量，彻底消灭切歌瞬间的满音量爆音。
      *   引入 y = x² 指数拟合曲线，修正人耳听觉的对数偏差，提供如同流媒体大厂般的丝滑音量推子体验。
  *   **离线数据灾备**
      *   歌单数据默认存储于本地 `localStorage`。
      *   支持将所有播放列表与元数据一键导出为 JSON 文件进行冷备份与跨设备恢复。

  ---

  ## 🛠️ 技术栈

  *   **前端表现层**: Vue 3 (Composition API) + Vite + TypeScript
  *   **系统桥接层**: Tauri (跨线程 IPC 通信与原生托盘支持)
  *   **后端逻辑层**: Rust + Tokio (全异步事件流调度)
  *   **物理引擎层**: `rodio` (音频输出) + `reqwest` (网络请求) + `FFmpeg` (媒体解码)

  ---

  ## 🚀 编译与运行

  ### 环境依赖
  1. 安装 [Node.js](https://nodejs.org/) (推荐 v18+)。
  2. 安装 [Rust](https://www.rust-lang.org/) 环境。
  3. 准备安全版本的 `ffmpeg.exe` (见下方安全声明)。

  ### 构建指令
  ```bash
  # 克隆仓库
  git clone [https://github.com/bloopes/bvplayer.git](https://github.com/bloopes/bvplayer.git)
  cd bvplayer
  
  # 安装前端依赖
  npm install
  
  # 本地开发调试
  npm run tauri dev
  
  # 构建生产环境安装包
  npm run tauri build

## v1.1.0 更新日志

### 🎨 界面与交互优化 (UI/UX)

- **丝滑的视图过渡**：引入全局级 `<FadeTransition>` 渐变过渡组件，重构了大厅界面与歌单详情页的切换逻辑，彻底消除路由切换时的 DOM 闪烁与排版坍塌。
- **物理级拖拽排序**：实现了丝滑的列表拖拽排序功能，增加物理按压反馈与幽灵悬浮动画，并确保拖拽过程中的数据流与后端状态机实时同步。
- **外部链接唤起**：修复了 Webview 沙盒环境下的链接跳转逻辑，现在通过 Tauri 底层 Shell API 可正确唤醒系统默认浏览器打开 GitHub 主页。
- **视觉重构与完善**：全新设计的系统设置与弹窗界面，深度适配现代毛玻璃与深色主题；音量控制滑块加入随音量动态填充的视觉效果。

### 🛠️ 核心稳定性与架构重构 (Backend Stability)

- **致命崩溃修复 (Crash Prevention)**：全面清除了 Rust 后端并发控制中的裸 `unwrap()` 调用。重写了 `AppState` 的锁管理机制，加入 Mutex 锁中毒恢复策略，彻底解决因设备热插拔或 FFmpeg 异常导致的整个应用连带闪退问题。
- **全局错误结构化 (Structured Error Handling)**：重构 API 层与指令路由层的错误传递机制。打通 `serde::Serialize`，所有的底层网络断联、I/O 读写错误、B站 API 业务异常均转化为结构化的 JSON 错误抛给前端，不再丢失报错上下文。
- **模块化解耦 (Architecture Decoupling)**：对原本承载超过 500 行的 `main.rs` 进行了深度拆分。分离出 `commands` (IPC 路由)、`daemon` (异步音频守护管道)、`models` (核心领域数据结构) 等独立模块，消除了复杂的网状依赖。
- **内存与性能精进 (Performance)**：
  - 优化 `PlaylistManager` 的内存访问，列表获取由深度克隆改为 `&[Song]` 切片借用。
  - `PlaybackMode` 枚举实现 `Copy` 特征，降低按值传递开销。
  - 随机播放模式 (Shuffle) 引入 `rand::Rng` 算法，消除了每次切歌时产生无意义的 `Vec` 堆分配。
  - 提取并复用了音频缓存准备逻辑 (`prepare_audio_cache`)，规范了预加载任务的异常处理闭环。
