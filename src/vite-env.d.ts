/**
 * TypeScript 环境类型声明。
 *
 * 为 Vite 和 Tauri 的构建时变量提供类型标注，
 * 使 `import.meta.env` 和 Tauri API 模块在 IDE 中获得智能提示。
 */
import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

/**
 * Vite 构建配置。
 *
 * - 固定端口 1420 供 Tauri dev 流程对接
 * - 根据目标平台动态调整浏览器兼容目标（Windows → Chrome 105，macOS → Safari 13）
 * - chunkSizeWarningLimit 上调至 1000KB 以抑制大型依赖的警告
 */
export default defineConfig(async () => ({
  plugins: [vue()],

  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  envPrefix: ["VITE_", "TAURI_"],

  build: {
    target: process.env.TAURI_PLATFORM == "windows" ? "chrome105" : "safari13",
    chunkSizeWarningLimit: 1000,
  },
}));
