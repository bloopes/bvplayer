import { ref } from 'vue';

type ToastType = 'success' | 'warning' | 'error';

const toastState = ref({
  visible: false,
  message: '',
  type: 'success' as ToastType
});

let toastTimer: ReturnType<typeof setTimeout> | null = null;

export function useToast() {
  const showToast = (message: string, type: ToastType = 'success') => {
    toastState.value = { visible: true, message, type };
    if (toastTimer) clearTimeout(toastTimer);
    toastTimer = setTimeout(() => { 
      toastState.value.visible = false; 
    }, 3000);
  };

  return { toastState, showToast };
}