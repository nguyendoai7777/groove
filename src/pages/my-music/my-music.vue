<template>
  <div class="p-6">
    <!-- Header -->
    <div class="flex pb-4 justify-between items-center mb-6">
      <h1 class="text-3xl font-light text-zinc-100">My Music</h1>
      <div class="flex gap-2">
        <icon-btn title="Settings" :size="{ icon: 'sm' }" src="Settings" />
      </div>
    </div>

    <!-- Import Banner (Aligns with the first image) -->
    <div
      @click="handleImportClick"
      class="flex items-start gap-3.5 p-4 bg-zinc-900/40 hover:bg-zinc-900/60 border border-zinc-800/80 hover:border-zinc-700/80 rounded-lg cursor-pointer transition-all duration-200 mb-8 max-w-xl select-none"
    >
      <div class="flex-shrink-0 mt-0.5">
        <svg-sprite src="Folder" class="w-5 h-5 text-zinc-400" />
      </div>
      <div class="flex flex-col">
        <span class="text-[13px] font-semibold text-zinc-300">Not finding everything?</span>
        <span class="text-xs text-cyan-400 hover:text-cyan-300 hover:underline mt-1 font-medium">
          Show us where to look for music
        </span>
      </div>
    </div>

    <!-- Music Items Grid -->
    <div>
      <div
        class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6"
      >
        <music-card
          v-for="item in musicItems"
          :key="item.id"
          :type="item.type"
          :title="item.title"
          :subtitle="item.subtitle"
          :thumbnail="item.thumbnail"
          :songs-count="item.songsCount"
          :accent-color="item.accentColor"
          @play="handlePlay"
          @settings="handleSettings"
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref } from 'vue'
  import IconBtn from '@groovex/ui/button/icon-btn.vue'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import { MusicCard } from '@groovex/ui/music-card'

  interface MusicItem {
    id: string
    type: 'album' | 'folder'
    title: string
    subtitle: string
    thumbnail?: string
    songsCount?: number
    accentColor?: string
  }

  // Mock data matching the user's screenshots
  const musicItems = ref<MusicItem[]>([
    {
      id: '1',
      type: 'album',
      title: 'Lệ Quyên',
      subtitle: 'Unknown Artist',
      thumbnail: '/images/le_quyen_cover.png',
      accentColor: '#8a5c2d', // warm brown/gold accent color matching cover
    },
    {
      id: '2',
      type: 'album',
      title: 'V Pop',
      subtitle: 'Unknown Artist',
      thumbnail: '/images/vpop_cover.png',
      accentColor: '#14b8a6', // teal accent color matching cover
    },
    {
      id: '3',
      type: 'folder',
      title: 'VRM',
      subtitle: 'Unknown Artist',
      accentColor: '#0ea5e9', // cyan/blue accent color
      songsCount: 248, // From the listed folder previously
    },
    {
      id: '4',
      type: 'album',
      title: 'Nonstop',
      subtitle: 'Unknown Artist',
      accentColor: '#4b5563', // grey accent color
    },
  ])

  // Triggers folder selection
  function handleImportClick() {
    console.log('Import triggered: opening folder selection via Tauri...')
    // Note for logic: When Tauri dialog returns folder path, Tauri scans the files inside.
    // It extracts the thumbnail of the first song found containing a picture frame (e.g. APIC in ID3),
    // saves it, inserts the album/folder record with this thumbnail path into the DB, and refreshes the list.
  }

  function handlePlay(item: { type: 'album' | 'folder'; title: string }) {
    console.log(`Play all triggered for ${item.type}: ${item.title}`)
  }

  function handleSettings(item: { type: 'album' | 'folder'; title: string }) {
    console.log(`Settings clicked for ${item.type}: ${item.title}`)
  }
</script>
