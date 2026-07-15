import { createApp } from 'vue';
import './assets/global.css'; // 确保你已将 style.css 移至此处
import App from './App.vue';

document.addEventListener('contextmenu', event => event.preventDefault());
createApp(App).mount('#app');