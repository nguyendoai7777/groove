<template>
  <div class="h-(--nav-head-h) w-full flex items-center relative select-none" data-tauri-drag-region>
    <!-- Back Button -->
    <icon-btn
      src="AngleLeft"
      data-tauri-no-drag
      :class="['transition-all duration-200', canGoBack ? 'opacity-100' : 'opacity-20 cursor-not-allowed pointer-events-none']"
      @click="handleBack" />

    <!-- Window Controls -->
    <div class="ml-auto flex items-center h-full" data-tauri-no-drag>
      <icon-btn title="Minimize" :size="{ icon: 'sm' }" src="Minus" @click="minimize" />
      <icon-btn v-if="isMaximized" title="Restore" :size="{ icon: 'sm' }" src="ArrowsCompress" @click="restore" />
      <icon-btn v-else title="Maximize" :size="{ icon: 'sm' }" src="ArrowsExpand" @click="maximize" />
      <icon-btn
        title="Close"
        :size="{ icon: 'sm' }"
        src="Close"
        class="hover:bg-red-600 hover:text-white transition-colors rounded-none"
        @click="close" />
    </div>

    <!-- Search Bar -->
    <div
      class="grx-SearchBar absolute top-1/2 left-1/2 -translate-1/2 w-search-bar-w cursor-pointer rounded-full border border-solid flex items-center justify-center py-1 select-none pointer-events-none">
      <div class="text-xs text-theme-text-disabled font-light">search all</div>
    </div>
  </div>
</template>

<style>
  .grx-SearchBar {
    border: var(--border-search);
    background: var(--bg-search);
  }
</style>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue'
  import { useRouter, useRoute } from 'vue-router'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import IconBtn from '@groovex/ui/button/icon-btn.vue'

  const router = useRouter()
  const route = useRoute()
  const appWindow = getCurrentWindow()

  const isMaximized = ref(false)

  const canGoBack = computed(() => {
    return route.query.view === 'list' || !!(window.history.state && window.history.state.back)
  })

  function handleBack() {
    if (canGoBack.value) {
      router.back()
    }
  }

  async function minimize() {
    await appWindow.minimize()
  }

  async function restore() {
    await appWindow.unmaximize()
    isMaximized.value = false
  }

  async function maximize() {
    await appWindow.maximize()
    isMaximized.value = true
  }

  async function close() {
    await appWindow.close()
  }

  onMounted(async () => {
    isMaximized.value = await appWindow.isMaximized()

    // Listen for resize to update maximized icon state
    appWindow.onResized(async () => {
      isMaximized.value = await appWindow.isMaximized()
    })
  })
</script>
