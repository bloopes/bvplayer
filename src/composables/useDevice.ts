// src/composables/useDevice.ts
import { ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';

const deviceList = ref<string[]>([]);
const currentDevice = ref<string>('');

export function useDevice() {
  const fetchDevices = async () => {
    try {
      // 严格声明后端返回的是字符串数组
      const devices = await invoke<string[]>('get_audio_devices');
      deviceList.value = devices;
      
      // 读取上次保存的设备
      const savedDevice = localStorage.getItem('bvplayer_device');
      if (savedDevice && devices.includes(savedDevice)) {
        currentDevice.value = savedDevice;
        await invoke("set_audio_device", { device: savedDevice });
      }
    } catch (e) {
      console.error('获取硬件设备失败:', e);
    }
  };

  const switchDevice = async (deviceName: string) => {
    try {
      localStorage.setItem('bvplayer_device', deviceName);
      currentDevice.value = deviceName;
      // 依次发送设置指令与实时切换指令
      await invoke('set_audio_device', { device: deviceName });
      await invoke('switch_device_realtime', { device: deviceName });
    } catch (e) {
      console.error('切换设备失败:', e);
    }
  };

  return {
    deviceList,
    currentDevice,
    fetchDevices,
    switchDevice
  };
}