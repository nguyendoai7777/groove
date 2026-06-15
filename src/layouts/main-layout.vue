<template>
  <div class="flex h-screen overflow-hidden">
    <side-nav />
    <div class="flex-1 flex flex-col h-full overflow-hidden bg-(--bg-main)">
      <app-bar class="" />
      <div class="flex-1 overflow-y-auto">
        <router-view />
      </div>
    </div>
  </div>
</template>

<script setup>
  import SideNav from '@groovex/ui/side-nav/side-nav'
  import AppBar from '@groovex/ui/app-bar/app-bar.vue'
</script>
