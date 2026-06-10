<template>
  <div class="flex">
    <side-nav />
    <div class="w-[stretch]">
      <app-bar />
      <router-view />
    </div>
  </div>
</template>

<script setup>
import SideNav from '@groovex/ui/side-nav/side-nav'
import AppBar from '@groovex/ui/app-bar/app-bar.vue'
</script>
