<template>
  <div
    class="flex flex-col min-h-full px-4 text-zinc-100 select-none"
    :style="{
      '--accent-bg': accentBgColor,
    }"
  >
    <!-- Background immersive glow -->
    <div
      class="absolute inset-0 -z-10 bg-linear-to-b from-(--accent-bg) to-zinc-950/20 opacity-25 pointer-events-none transition-all duration-500"
    ></div>

    <!-- Category Header Info -->
    <div
      class="sticky top-0 py-3 z-20 flex transition-all duration-300 ease-in-out"
      :class="[
        isScrolled
          ? 'flex-row items-center gap-4 py-3 backdrop-blur-md -mx-4 px-4'
          : 'flex-col md:flex-row gap-6 items-start md:items-end pb-2 mb-8',
      ]"
    >
      <!-- Big cover image -->
      <div
        class="relative rounded-xl overflow-hidden bg-zinc-800 shadow-xl shrink-0 transition-all duration-300 ease-in-out"
        :class="isScrolled ? 'w-20 h-20 shadow-md' : 'w-40 h-40 shadow-xl'"
      >
        <img v-if="thumbnail" :src="thumbnail" :alt="title" class="w-full h-full object-cover" />
        <div
          v-else
          class="w-full h-full flex items-center justify-center transition-all duration-300 ease-in-out"
          :class="placeholderClass"
        >
          <svg-sprite
            :src="type === 'folder' ? 'Play' : 'Album'"
            :class="[
              type === 'folder' ? 'text-white/90 drop-shadow-lg' : 'text-zinc-600',
              isScrolled ? 'w-8 h-8' : 'w-16 h-16',
            ]"
            class="transition-all duration-300 ease-in-out"
          />
        </div>
      </div>

      <!-- Text details -->
      <div class="flex-1 min-w-0">
        <span
          class="text-xs text-cyan-400 font-bold uppercase tracking-widest transition-all duration-300 block"
          :class="isScrolled ? 'opacity-0 h-0 overflow-hidden mb-0' : 'mb-1'"
        >
          {{ type === 'folder' ? 'Folder' : 'Album' }}
        </span>
        <h2
          class="font-extrabold truncate text-white transition-all duration-300"
          :class="isScrolled ? 'text-lg md:text-xl mt-0' : 'text-3xl md:text-4xl mt-1'"
        >
          {{ title }}
        </h2>
        <p
          class="text-zinc-400 flex items-center gap-2 transition-all duration-300 mt-1"
          :class="isScrolled ? 'text-xs md:text-sm' : 'text-sm md:mt-2'"
        >
          <span v-if="!isScrolled" class="transition-opacity duration-300">
            {{ displaySubtitle }}
          </span>
          <span v-if="!isScrolled && songs.length > 0" class="text-zinc-600">•</span>

          <span v-if="isScrolled" class="font-semibold text-cyan-400 uppercase tracking-wider">
            {{ type === 'folder' ? 'Folder' : 'Album' }}
          </span>
          <span v-if="isScrolled" class="text-zinc-600">•</span>

          <span v-if="songs.length > 0">{{ songs.length }} items</span>
        </p>

        <!-- Quick actions -->
        <div
          class="flex gap-3 transition-all duration-300 ease-in-out overflow-hidden"
          :class="
            isScrolled ? 'max-h-0 opacity-0 mt-0 pointer-events-none' : 'max-h-12 opacity-100 mt-4'
          "
        >
          <button
            class="flex items-center gap-2 px-5 py-2 rounded-full bg-cyan-600 hover:bg-cyan-500 text-white font-semibold text-sm transition-all duration-150 shadow-md hover:scale-[1.02] active:scale-95 cursor-pointer"
            @click="handlePlayAll"
            :disabled="songs.length === 0"
            :class="{ 'opacity-50 cursor-not-allowed': songs.length === 0 }"
          >
            <svg-sprite src="Play" class="w-4 h-4 fill-white" />
            Play all
          </button>
        </div>
      </div>
    </div>

    <!-- Songs Table -->
    <div class="flex-1 bg-zinc-900/10 rounded-xl overflow-hidden">
      <!-- Loading State -->
      <div v-if="isLoading" class="flex flex-col items-center justify-center py-20 gap-3">
        <div
          class="w-8 h-8 rounded-full border-2 border-cyan-500/20 border-t-cyan-500 animate-spin"
        ></div>
        <span class="text-xs text-zinc-500">Loading songs...</span>
      </div>

      <!-- Empty State -->
      <div
        v-else-if="songs.length === 0"
        class="flex flex-col items-center justify-center py-20 text-center"
      >
        <svg-sprite src="Song" class="w-12 h-12 text-zinc-600 mb-3" />
        <span class="text-sm font-semibold text-zinc-400">This category is empty</span>
        <span class="text-xs text-zinc-600 mt-1">Scan folder to add songs</span>
      </div>

      <!-- Table Content -->
      <div v-else class="overflow-x-auto">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="text-zinc-500 text-xs font-semibold uppercase tracking-wider">
              <th class="py-3.5 px-4 w-12 text-center">#</th>
              <th class="py-3.5 px-4">Filename</th>
              <th class="py-3.5 px-4 hidden sm:table-cell">Title (Metadata)</th>
              <th class="py-3.5 px-4 hidden md:table-cell">Artist</th>
              <th class="py-3.5 px-4 w-24 text-right">Duration</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(song, idx) in songs"
              :key="song.id"
              :data-src="song.file_path"
              class="group/row hover:bg-gray-400/10 border-b border-zinc-900/40 transition-colors duration-150 text-sm cursor-pointer"
              @click="handlePlaySong(song)"
            >
              <td
                class="py-3 px-4 text-center text-zinc-500 group-hover/row:text-cyan-400 transition-colors"
              >
                <!-- Play symbol on hover, index otherwise -->
                <span class="group-hover/row:hidden">{{ idx + 1 }}</span>
                <svg-sprite
                  src="Play"
                  class="w-3 h-3 mx-auto hidden group-hover/row:block text-cyan-400 fill-cyan-400"
                />
              </td>
              <td
                class="py-3 px-4 font-medium text-zinc-200 group-hover/row:text-white truncate max-w-xs sm:max-w-sm"
              >
                {{ song.filename }}
              </td>
              <td class="py-3 px-4 text-zinc-400 hidden sm:table-cell truncate max-w-xs">
                {{ song.title || '—' }}
              </td>
              <td class="py-3 px-4 text-zinc-400 hidden md:table-cell truncate max-w-xs">
                {{ song.artist || '—' }}
              </td>
              <td class="py-3 px-4 text-right text-zinc-400 tabular-nums">
                {{ formatDuration(song.duration) }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted } from 'vue'
  import { invoke } from '@tauri-apps/api/core'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import { useLayoutScroll } from '../../shared/composables/use-layout-scroll'
  import { formatDuration } from '@groovex/core'
  import { useAudioPlayer } from '@groovex/state'
  import type { Song } from '@groovex/types'

  const props = defineProps<{
    type: 'album' | 'folder'
    id: number
    title: string
    subtitle?: string
    thumbnail?: string
    accentColor?: string
    songsCount?: number
  }>()

  defineEmits<{
    (e: 'back'): void
  }>()

  const songs = ref<Song[]>([])
  const isLoading = ref(true)

  const isScrolled = ref(false)

  useLayoutScroll((instance) => {
    const { viewport } = instance.elements()
    isScrolled.value = viewport.scrollTop > 40
  })

  // Accent glow background color
  const accentBgColor = computed(() => {
    return props.accentColor || 'rgba(6, 182, 212, 0.4)'
  })

  // Subtitle string
  const displaySubtitle = computed(() => {
    if (props.subtitle) return props.subtitle
    if (props.type === 'folder') return 'Local Folder'
    return 'Unknown Artist'
  })

  // Fallback placeholder class
  const placeholderClass = computed(() => {
    if (props.type === 'folder') {
      return 'bg-gradient-to-br from-cyan-500 to-blue-600 shadow-[inset_0_2px_10px_rgba(255,255,255,0.15)]'
    }
    return 'bg-zinc-800 border border-zinc-700/50'
  })

  // Fetch category songs on mount
  onMounted(async () => {
    try {
      songs.value = await invoke<Song[]>('get_category_songs', {
        categoryType: props.type,
        categoryId: props.id,
      })
    } catch (err) {
      console.error('Error fetching category songs:', err)
    } finally {
      isLoading.value = false
    }
  })

  const player = useAudioPlayer()

  function handlePlayAll() {
    if (songs.value.length === 0) return
    const playlist = songs.value.map((s) => ({
      ...s,
      thumbnail: props.thumbnail,
    }))
    player.playSong(playlist[0], playlist)
  }

  function handlePlaySong(song: Song) {
    const playlist = songs.value.map((s) => ({
      ...s,
      thumbnail: props.thumbnail,
    }))
    const playSongItem = playlist.find((s) => s.id === song.id) || {
      ...song,
      thumbnail: props.thumbnail,
    }
    player.playSong(playSongItem, playlist)
  }
</script>
