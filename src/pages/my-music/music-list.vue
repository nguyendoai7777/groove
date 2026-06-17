<template>
  <div
    class="flex flex-col min-h-full px-4 text-theme-text select-none"
    :style="{
      '--accent-bg': accentBgColor,
    }">
    <!-- Background immersive glow -->
    <div
      class="absolute top-0 left-0 right-0 h-music-list-glow-h -z-10 bg-linear-to-b from-(--accent-bg) to-transparent opacity-40 pointer-events-none transition-all duration-500"></div>

    <!-- Category Header Info -->
    <div
      class="sticky top-0 py-3 z-20 flex transition-all duration-300 ease-in-out"
      :class="[
        isScrolled
          ? 'flex-row items-center gap-4 py-3 backdrop-blur-md -mx-4 px-4'
          : 'flex-col md:flex-row gap-6 items-start md:items-end pb-2 mb-8',
      ]">
      <!-- Big cover image -->
      <div
        class="relative rounded-xl overflow-hidden bg-theme-bg-placeholder shadow-xl shrink-0 transition-all duration-300 ease-in-out"
        :class="isScrolled ? 'w-20 h-20 shadow-md' : 'w-40 h-40 shadow-xl'">
        <img v-if="thumbnail" :src="thumbnail" :alt="title" class="w-full h-full object-cover" />
        <div
          v-else
          class="w-full h-full flex items-center justify-center transition-all duration-300 ease-in-out"
          :class="placeholderClass">
          <svg-sprite
            :src="type === 'folder' ? 'Play' : 'Album'"
            :class="[
              type === 'folder' ? 'text-theme-text-on-accent drop-shadow-lg' : 'text-theme-text-disabled',
              isScrolled ? 'w-8 h-8' : 'w-16 h-16',
            ]"
            class="transition-all duration-300 ease-in-out" />
        </div>
      </div>

      <!-- Text details -->
      <div class="flex-1 min-w-0">
        <span
          class="text-xs text-theme-accent-light font-bold tracking-widest transition-all duration-300 block"
          :class="isScrolled ? 'opacity-0 h-0 overflow-hidden mb-0' : 'mb-1'">
          {{ type === 'folder' ? 'Folder' : 'Album' }}
        </span>
        <h2
          class="font-extrabold truncate text-white transition-all duration-300"
          :class="isScrolled ? 'text-lg md:text-xl mt-0' : 'text-3xl md:text-4xl mt-1'">
          {{ title }}
        </h2>
        <p
          class="text-theme-text-muted flex items-center gap-2 transition-all duration-300 mt-1"
          :class="isScrolled ? 'text-xs md:text-sm' : 'text-sm md:mt-2'">
          <span v-if="!isScrolled" class="transition-opacity duration-300">
            {{ displaySubtitle }}
          </span>
          <span v-if="!isScrolled && songs.length > 0" class="text-theme-text-disabled">•</span>

          <span v-if="isScrolled" class="font-semibold text-theme-accent-light tracking-wider">
            {{ type === 'folder' ? 'Folder' : 'Album' }}
          </span>
          <span v-if="isScrolled" class="text-theme-text-disabled">•</span>

          <span v-if="songs.length > 0">{{ songs.length }} items</span>
        </p>

        <!-- Quick actions -->
        <div
          class="flex gap-3 transition-all duration-300 ease-in-out overflow-hidden"
          :class="isScrolled ? 'max-h-0 opacity-0 mt-0 pointer-events-none' : 'max-h-12 opacity-100 mt-4'">
          <custom-btn
            variant="primary"
            rounded="full"
            size="md"
            @click="handlePlayAll"
            :disabled="songs.length === 0"
            class="flex items-center gap-2">
            <svg-sprite src="Play" class="w-4 h-4" />
            Play all
          </custom-btn>
        </div>
      </div>
    </div>

    <!-- Songs Table -->
    <div class="flex-1 bg-theme-bg-card overflow-hidden">
      <!-- Loading State -->
      <div v-if="isLoading" class="flex flex-col items-center justify-center py-20 gap-3">
        <div class="w-8 h-8 rounded-full border-2 border-theme-accent/20 border-t-theme-accent animate-spin"></div>
        <span class="text-xs text-theme-text-disabled">Loading songs...</span>
      </div>

      <!-- Empty State -->
      <div v-else-if="songs.length === 0" class="flex flex-col items-center justify-center py-20 text-center">
        <svg-sprite src="Song" class="w-12 h-12 text-theme-text-disabled mb-3" />
        <span class="text-sm font-semibold text-theme-text-muted">This category is empty</span>
        <span class="text-xs text-theme-text-disabled mt-1">Scan folder to add songs</span>
      </div>

      <!-- Table Content -->
      <div v-else class="overflow-x-auto">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="text-theme-text-disabled text-xs font-semibold uppercase tracking-wider">
              <th class="py-3.5 px-4 w-12 text-center">#</th>
              <th class="py-3.5 px-4">Name</th>
              <th class="py-3.5 px-4 w-24 text-left min-w-50">Duration</th>
              <th class="py-3.5 px-4 hidden md:table-cell">Artist</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(song, idx) in songs"
              :key="song.id"
              :data-src="song.file_path"
              class="group/row hover:bg-gray-400/10 border-b border-theme-border/40 transition-colors duration-150 text-sm cursor-pointer"
              @click="handlePlaySong(song)">
              <td class="py-3 px-4 text-center text-theme-text-disabled group-hover/row:text-theme-accent-light transition-colors w-12">
                <div class="relative flex items-center justify-center w-full h-full">
                  <!-- If this song is NOT currently selected/playing in the player -->
                  <template v-if="player.currentSong?.id !== song.id">
                    <!-- Default state: show index number -->
                    <span class="group-hover/row:hidden font-mono text-xs">{{ idx + 1 }}</span>
                    <!-- Hover state: show Play icon -->
                    <svg-sprite
                      src="Play"
                      class="w-3 h-3 mx-auto hidden group-hover/row:block text-theme-accent-light fill-theme-accent-light" />
                  </template>

                  <!-- If this song IS currently selected/playing in the player -->
                  <template v-else>
                    <!-- Default state: show bouncing visualizer SVG -->
                    <div class="group-hover/row:hidden flex items-center justify-center text-theme-accent-light">
                      <playing-visualizer :paused="!player.isPlaying" class="w-3.5 h-3.5" />
                    </div>
                    <!-- Hover state: show Play/Pause icon depending on state -->
                    <div class="hidden group-hover/row:block text-theme-accent-light">
                      <!-- If playing, show Pause icon -->
                      <svg
                        v-if="player.isPlaying"
                        class="w-3.5 h-3.5 fill-theme-accent-light mx-auto"
                        viewBox="0 0 24 24"
                        xmlns="http://www.w3.org/2000/svg">
                        <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
                      </svg>
                      <!-- If paused, show Play icon -->
                      <svg-sprite v-else src="Play" class="w-3 h-3 mx-auto text-theme-accent-light fill-theme-accent-light" />
                    </div>
                  </template>
                </div>
              </td>

              <td
                class="py-3 px-4 font-medium truncate max-w-xs sm:max-w-sm transition-colors duration-150"
                :class="
                  player.currentSong?.id === song.id ? 'text-theme-accent-light' : 'text-theme-text-secondary group-hover/row:text-white'
                ">
                {{ song.filename.replace(/\.[^/.]+$/, '') }}
              </td>
              <td class="py-3 px-4 text-left text-theme-text-muted tabular-nums">
                {{ formatDuration(song.duration) }}
              </td>

              <td class="py-3 px-4 text-theme-text-muted hidden md:table-cell truncate max-w-xs">
                {{ song.artist || '—' }}
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
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
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

  const dynamicAccentColor = ref(props.accentColor || 'rgba(6, 182, 212, 0.4)')

  // Accent glow background color
  const accentBgColor = computed(() => {
    return dynamicAccentColor.value
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
      return 'bg-gradient-to-br from-theme-accent to-theme-accent-secondary shadow-music-placeholder-inset'
    }
    return 'bg-theme-bg-placeholder border border-theme-border/50'
  })

  // Helper function to extract average dominant color from image URL or base64
  function getAverageColor(imageUrl: string): Promise<string> {
    return new Promise((resolve) => {
      const img = new Image()
      img.crossOrigin = 'Anonymous'
      img.onload = () => {
        const canvas = document.createElement('canvas')
        canvas.width = 1
        canvas.height = 1
        const ctx = canvas.getContext('2d')
        if (ctx) {
          ctx.drawImage(img, 0, 0, 1, 1)
          const pixel = ctx.getImageData(0, 0, 1, 1).data
          let r = pixel[0]
          let g = pixel[1]
          let b = pixel[2]

          // Keep brightness under control for dark mode text readability
          const brightness = (r * 299 + g * 587 + b * 114) / 1000
          if (brightness > 120) {
            const factor = 120 / brightness
            r = Math.round(r * factor)
            g = Math.round(g * factor)
            b = Math.round(b * factor)
          }
          resolve(`rgb(${r}, ${g}, ${b})`)
        } else {
          resolve('#06b6d4')
        }
      }
      img.onerror = () => resolve('#06b6d4')
      img.src = imageUrl
    })
  }

  // Fetch category songs on mount
  onMounted(async () => {
    if (props.thumbnail) {
      try {
        const avgColor = await getAverageColor(props.thumbnail)
        dynamicAccentColor.value = avgColor
      } catch (e) {
        console.error('Failed to extract dominant color:', e)
      }
    }
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
    if (player.currentSong?.id === song.id) {
      player.togglePlay()
      return
    }
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
