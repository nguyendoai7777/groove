<script setup lang="ts">
  import KeyboardPalette from '@groovex/ui/commands/keyboard-palette.vue'
  import { useKeyboardShortcuts } from './shared/composables/global-event-listener-top'
  import AudioController from './components/audio-controller/audio-controller.vue'
  import { useToast } from './shared/composables/use-toast'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'

  const { initShortcuts } = useKeyboardShortcuts()
  initShortcuts()

  const { toasts, remove } = useToast()
</script>
<template>
  <router-view />
  <keyboard-palette />
  <audio-controller />

  <!-- Global Toasts Container -->
  <div class="fixed top-4 right-4 z-99999 flex flex-col gap-2.5 pointer-events-none max-w-sm w-[calc(100%-2rem)]">
    <transition-group name="toast">
      <div
        v-for="toast in toasts"
        :key="toast.id"
        class="pointer-events-auto bg-theme-bg-card/35 backdrop-blur-md border border-border-theme text-theme-text rounded-xl px-4 py-3.5 shadow-xl flex items-center justify-between transition-all duration-300 transform select-none">
        <div class="flex items-center gap-2.5 min-w-0 flex-1">
          <svg-sprite src="Folder" class="w-4 h-4 text-theme-accent-light shrink-0" />
          <span class="text-xs font-semibold leading-snug text-theme-text-secondary">{{ toast.message }}</span>
        </div>
        <button
          @click="remove(toast.id)"
          class="text-theme-text-disabled hover:text-theme-text ml-3 shrink-0 p-0.5 hover:bg-white/5 rounded-md transition-colors cursor-pointer">
          <svg-sprite src="Close" class="w-3.5 h-3.5" />
        </button>
      </div>
    </transition-group>
  </div>
</template>

<style>
  .toast-enter-active,
  .toast-leave-active {
    transition: all 0.3s cubic-bezier(0.25, 1, 0.5, 1);
  }
  .toast-enter-from {
    opacity: 0;
    transform: translateY(-20px) scale(0.95);
  }
  .toast-leave-to {
    opacity: 0;
    transform: translateY(-10px) scale(0.95);
  }
</style>
