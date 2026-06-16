<script setup lang="ts">
  import KeyboardPalette from '@groovex/ui/commands/keyboard-palette.vue'
  import { useKeyboardShortcuts } from './shared/composables/global-event-listener-top'
  import AudioController from './components/audio-controller/audio-controller.vue'

  const { initShortcuts } = useKeyboardShortcuts()
  initShortcuts()
</script>
<template>
  <router-view />
  <keyboard-palette />
  <audio-controller />
</template>
