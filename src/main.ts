/**
 * bvplayer 前端入口。
 *
 * 挂载 Vue 3 根组件到 `#app`，并加载全局样式。
 * 所有 IPC 通信、状态管理和业务逻辑均在 App.vue 中内聚。
 */
import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";

createApp(App).mount("#app");