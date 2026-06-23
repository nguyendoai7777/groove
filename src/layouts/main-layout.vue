<template>
  <div class="flex h-screen overflow-hidden">
    <side-nav />
    <div class="flex-1 flex flex-col h-full overflow-hidden bg-(--bg-main)">
      <app-bar class="" />
      <overlay-scrollbars-component
        :options="{ scrollbars: { autoHide: 'scroll' } }"
        ref="osComponentRef"
        defer
        class="flex-1 overflow-y-auto scroll-zone"
        @os-initialized="onOsInitialized">
        <router-view />
      </overlay-scrollbars-component>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref } from 'vue'
  import SideNav from '@groovex/ui/side-nav/side-nav.vue'
  import AppBar from '@groovex/ui/app-bar/app-bar.vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { provideLayoutScroll } from '@groovex/composables'

  const osInstance = ref<any>(null)

  function onOsInitialized(instance: any) {
    osInstance.value = instance
  }

  provideLayoutScroll(osInstance)
</script>

<style>
  .scroll-zone {
    max-height: calc(100vh - var(--nav-head-h) - var(--audio-controller-h));
  }
</style>
