<script setup lang="ts">
  import KeyboardPalette from '@groovex/ui/commands/keyboard-palette.vue'
  import { useKeyboardShortcuts } from './shared/composables/global-event-listener-top'

  const { initShortcuts } = useKeyboardShortcuts()
  initShortcuts()
</script>
<template>
  <router-view />
  <keyboard-palette />
</template>
