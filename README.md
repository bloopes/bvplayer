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
  git clone [https://github.com/your-username/bvplayer.git](https://github.com/your-username/bvplayer.git)
  cd bvplayer
  
  # 安装前端依赖
  npm install
  
  # 本地开发调试
  npm run tauri dev
  
  # 构建生产环境安装包
  npm run tauri build
