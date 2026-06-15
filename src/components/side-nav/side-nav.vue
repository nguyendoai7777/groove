<template>
  <div class="min-w-57.5 min-h-svh bg-(--nav-bg) h-full" style="backdrop-filter: blur(23px)">
    <div
      class="h-(--nav-head-h) duration-200 hover:bg-[#242424FF] flex items-center px-2 select-none font-light text-xs cursor-default"
      data-tauri-drag-region
    >
      {{ appInfo.name }}
    </div>
    <RouterLink v-for="nav in APP_ROUTES[0].children" :to="nav.path">
      <v-btn class="w-full flex justify-start px-2 text-sm h-(--nav-link-h) shadow-none">{{
        nav.meta['title']
      }}</v-btn>
    </RouterLink>
  </div>
</template>

<script setup>
  import appInfo from '../../../app.json' with { type: 'json' }
  import { APP_ROUTES } from '../../app.route.ts'
</script>
