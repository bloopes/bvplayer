<script setup lang="ts">
// 这是一个纯粹的包裹组件（Wrapper），不需要复杂的 script 逻辑
// 这里暴露一个可选的 mode 属性，默认为 "out-in"（先出后进），防止布局坍塌
defineProps({
  mode: {
    type: String as () => 'out-in' | 'in-out' | 'default',
    default: 'out-in'
  }
});
</script>

<template>
  <transition name="fade" :mode="mode === 'default' ? undefined : mode">
    <!-- slot 用于接收包裹在内部的任何组件或 HTML -->
    <slot></slot>
  </transition>
</template>

<style scoped>
/* 进场和退场的持续时间与极度平滑的缓动曲线 */
.fade-enter-active,
.fade-leave-active {
  transition: all 0.3s cubic-bezier(0.25, 0.8, 0.25, 1);
}

/* 进场前的初始状态，以及退场后的结束状态 */
.fade-enter-from,
.fade-leave-to {
  opacity: 0;
  transform: translateY(8px); /* 微微下沉，带一点浮现感 */
}

/* 进场后的结束状态，以及退场前的初始状态 */
.fade-enter-to,
.fade-leave-from {
  opacity: 1;
  transform: translateY(0);
}
</style>