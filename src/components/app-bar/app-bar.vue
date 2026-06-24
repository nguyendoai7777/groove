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
      class="absolute top-1/2 left-1/2 -translate-1/2 w-search-bar-w cursor-pointer rounded-full border border-solid flex items-center justify-between px-3 py-1 select-none transition-all duration-200 gap-2"
      :class="[
        commandPalette.isOpen
          ? 'bg-(--input-border-bg) border-(--input-border-focus) shadow-[0_0_0_2px_rgba(6,182,212,0.15)]'
          : 'bg-(--bg-search) border-(--color-border-search) hover:border-(--input-border-color-hover) hover:bg-zinc-800/40',
      ]"
      @click="commandPalette.open()">
      <div class="flex items-center gap-2 overflow-hidden flex-1">
        <svg-sprite
          src="Search"
          class="w-3.5 h-3.5 text-theme-text-disabled shrink-0 transition-colors duration-200"
          :class="{ 'text-(--input-border-focus)': commandPalette.isOpen }" />
        <span class="text-xs text-theme-text-disabled font-light truncate">Search songs, commands...</span>
      </div>
      <div class="flex items-center gap-0.5 text-[10px] text-theme-text-disabled shrink-0 select-none">
        <kbd class="px-1.5 py-px rounded bg-zinc-700/30 border border-zinc-600/30 font-sans shadow-sm">Ctrl</kbd>
        <span class="opacity-50">+</span>
        <kbd class="px-1.5 py-px rounded bg-zinc-700/30 border border-zinc-600/30 font-sans shadow-sm">K</kbd>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue';
  import { useRouter, useRoute } from 'vue-router';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import IconBtn from '@groovex/ui/button/icon-btn.vue';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import { useCommandPaletteStore } from '@groovex/store';

  const router = useRouter();
  const route = useRoute();
  const appWindow = getCurrentWindow();
  const commandPalette = useCommandPaletteStore();

  const isMaximized = ref(false);

  const canGoBack = computed(() => {
    return route.query.view === 'list' || !!(window.history.state && window.history.state.back);
  });

  function handleBack() {
    if (canGoBack.value) {
      router.back();
    }
  }

  async function minimize() {
    await appWindow.minimize();
  }

  async function restore() {
    await appWindow.unmaximize();
    isMaximized.value = false;
  }

  async function maximize() {
    await appWindow.maximize();
    isMaximized.value = true;
  }

  async function close() {
    await appWindow.close();
  }

  onMounted(async () => {
    isMaximized.value = await appWindow.isMaximized();

    // Listen for resize to update maximized icon state
    appWindow.onResized(async () => {
      isMaximized.value = await appWindow.isMaximized();
    });
  });
</script>
