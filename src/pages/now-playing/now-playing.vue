<template>
  <div class="now-playing-page h-now-playing-h p-4 text-theme-text flex gap-5 relative select-none overflow-hidden">
    <!-- Immersive Dynamic Background Glow -->
    <div
      class="absolute top-0 left-0 right-0 h-now-playing-glow-h -z-10 bg-linear-to-b from-(--dynamic-accent) to-transparent opacity-30 pointer-events-none transition-all duration-700 ease-out"
      :style="{ '--dynamic-accent': dynamicAccentColor }"></div>

    <!-- Left side: Album Art & Controls Info -->
    <div
      class="w-1/4 min-w-now-playing-left-min-w max-w-now-playing-left-max-w flex flex-col items-center justify-start pt-4 shrink-0 @container">
      <!-- Album Cover Card -->
      <div
        class="relative group w-full aspect-square rounded-2xl overflow-hidden shadow-now-playing-cover bg-theme-bg-placeholder transition-all duration-500 hover:scale-[1.02] hover:shadow-now-playing-cover-hover">
        <img
          v-if="thumbnailUrl"
          :src="thumbnailUrl"
          alt="Now Playing Cover"
          class="w-full h-full object-cover transition-transform duration-700 group-hover:scale-105" />
        <div v-else class="w-full h-full flex items-center justify-center bg-linear-to-br from-theme-border-hover to-theme-bg-item">
          <svg-sprite src="Album" class="w-16 h-16 text-theme-text-disabled" />
        </div>
      </div>

      <!-- Metadata -->
      <div class="text-center mt-5 w-full px-2">
        <h1 class="text-base font-bold truncate text-white mb-1" id="now-playing-title">
          {{ songTitle }}
        </h1>
        <p class="text-theme-text-muted text-xs truncate">{{ artistName }}</p>
      </div>

      <!-- Simple visualizer animation -->
      <div class="flex items-center gap-1.5 h-6 mt-4">
        <playing-visualizer :paused="!isPlaying" class="w-5 h-5 text-theme-accent-light" />
      </div>
    </div>

    <!-- Middle side: Lyrics View & Editor -->
    <div class="flex-1 min-w-0 flex flex-col h-full backdrop-blur-xs overflow-hidden rounded-2xl border border-theme-border/50 py-2">
      <!-- View Mode -->
      <template v-if="!isEditingLyrics">
        <div class="flex items-center justify-between mb-3 border-b border-theme-border/40 pb-2.5 shrink-0 relative">
          <span class="text-2xl font-semibold tracking-wider absolute top-1/2 left-1/2 -translate-1/2">Lyrics</span>
          <v-btn
            v-slot:default
            v-if="currentSong"
            @click="startEditLyrics"
            class="ml-auto mr-2 flex rounded-full items-center gap-1.5 px-3 py-1 bg-theme-bg-placeholder/50 hover:bg-theme-border-hover/50 text-xs text-theme-accent-light font-semibold border border-theme-border-hover/30 hover:scale-[1.02] active:scale-95 text-none h-auto shadow-none cursor-pointer"
            variant="flat">
            <div class="flex items-center gap-1.5 py-0.5">
              <svg-sprite src="Edit" class="w-3.5 h-3.5" />
              <span>{{ lyricLines.length > 0 ? 'Edit Lyrics' : 'Add Lyrics' }}</span>
            </div>
          </v-btn>
        </div>

        <!-- No song loaded state -->
        <div v-if="!currentSong" class="flex flex-col items-center justify-center py-20 text-center flex-1">
          <svg-sprite src="HelpCircle" class="w-10 h-10 text-theme-border-hover mb-3" />
          <span class="text-xs font-semibold text-theme-text-disabled">Play a song to view lyrics</span>
        </div>

        <!-- Lyrics content -->
        <overlay-scrollbars-component
          v-else-if="lyricLines.length > 0"
          :options="{ scrollbars: { autoHide: 'scroll' } }"
          defer
          class="flex-1 pr-1 text-center py-2 px-6">
          <div class="flex flex-col gap-1.5">
            <p
              v-for="(line, idx) in lyricLines"
              :key="idx"
              class="text-xs md:text-sm font-semibold py-0.5 text-theme-text-secondary hover:text-white transition-colors duration-150"
              :class="{ 'h-4': line.trim() === '' }">
              {{ line }}
            </p>
          </div>
        </overlay-scrollbars-component>

        <!-- Empty State -->
        <div v-else class="flex flex-col items-center justify-center py-20 text-center flex-1">
          <svg-sprite src="LyricsBubble" class="w-10 h-10 text-theme-border-hover mb-3" />
          <span class="text-xs font-semibold text-theme-text-muted">No lyrics available for this song</span>
        </div>
      </template>

      <!-- Edit Mode -->
      <template v-else>
        <div class="flex flex-col gap-3 flex-1 h-full">
          <div class="flex items-center justify-between border-b border-theme-border/40 pb-2.5 shrink-0">
            <span class="text-xs text-theme-text-muted font-semibold uppercase tracking-wider">
              {{ currentSong?.lyrics ? 'Edit Lyrics' : 'Add Lyrics' }}
            </span>
            <span class="text-[10px] text-theme-text-disabled">Lưu vào file & database</span>
          </div>

          <!-- Textarea -->
          <textarea
            v-model="lyricsDraft"
            placeholder="Paste or write lyrics here..."
            class="w-full flex-1 bg-theme-bg-item/40 border border-theme-border focus:border-theme-accent/50 rounded-xl p-3 text-xs text-theme-text-secondary outline-hidden resize-none font-sans leading-relaxed custom-scrollbar transition-colors focus:ring-1 focus:ring-theme-accent/20"></textarea>

          <!-- Action buttons -->
          <div class="flex items-center justify-end gap-3 mt-1 shrink-0">
            <custom-btn variant="secondary" @click="cancelEditLyrics" :disabled="isSavingLyrics">Cancel</custom-btn>
            <custom-btn variant="primary" @click="handleSaveLyrics" :loading="isSavingLyrics">Save</custom-btn>
          </div>
        </div>
      </template>
    </div>

    <!-- Right side: Playlist Queue List -->
    <div
      class="w-now-playing-right-w min-w-now-playing-right-min-w max-w-now-playing-right-max-w flex flex-col h-full backdrop-blur-xs p-4 rounded-2xl border border-theme-border/50 overflow-hidden">
      <div class="flex items-center justify-between mb-3 pb-2 border-b border-theme-border/30 shrink-0">
        <h3 class="text-sm font-bold text-white tracking-wide flex items-center gap-2">
          <span>Queue / Playlist</span>
          <span class="text-[10px] font-normal text-theme-text-disabled">({{ playlist.length }} tracks)</span>
        </h3>
      </div>

      <!-- Queue List Viewport -->
      <overlay-scrollbars-component
        :options="{ scrollbars: { autoHide: 'scroll' } }"
        defer
        class="flex-1 pr-1"
        @os-initialized="onQueueListOsInitialized">
        <div class="flex flex-col gap-1.5">
          <template v-if="playlist.length > 0">
            <div
              v-for="(song, idx) in playlist"
              :key="song.id + '-' + idx"
              :id="'queue-item-' + song.id"
              :ref="(el) => setActiveRowRef(el, idx)"
              class="group/item flex items-center gap-2.5 p-2 rounded-xl transition-all duration-200 cursor-pointer border border-transparent"
              :class="[
                idx < currentIndex ? 'opacity-25 hover:opacity-50' : 'opacity-100',
                idx === currentIndex
                  ? 'bg-theme-accent/10 border-theme-accent/30 shadow-md text-white'
                  : 'hover:bg-theme-bg-placeholder/40 text-theme-text-secondary',
              ]"
              @click="handlePlaySongAt(idx)">
              <!-- Index / Status -->
              <div class="w-6 flex items-center justify-center text-[10px] font-mono text-theme-text-disabled">
                <span v-if="idx !== currentIndex" class="group-hover/item:hidden">{{ idx + 1 }}</span>
                <svg-sprite
                  v-if="idx !== currentIndex"
                  src="Play"
                  class="w-2.5 h-2.5 text-theme-accent-light fill-theme-accent-light hidden group-hover/item:block" />
                <playing-visualizer v-if="idx === currentIndex" :paused="!isPlaying" class="w-3 h-3 text-theme-accent-light" />
              </div>

              <!-- Thumbnail -->
              <div class="w-8 h-8 rounded-md overflow-hidden bg-theme-bg-placeholder shrink-0 shadow-xs">
                <img v-if="song.thumbnail" :src="song.thumbnail" class="w-full h-full object-cover" />
                <div v-else class="w-full h-full flex items-center justify-center bg-theme-bg-item">
                  <svg-sprite src="Album" class="w-4 h-4 text-theme-text-disabled" />
                </div>
              </div>

              <!-- Title & Artist -->
              <div class="flex-1 min-w-0 text-left">
                <div
                  class="text-xs font-semibold truncate"
                  :class="idx === currentIndex ? 'text-theme-accent-light' : 'text-theme-text-secondary group-hover/item:text-white'">
                  {{ song.title || song.filename.replace(/\.[^/.]+$/, '') }}
                </div>
                <div class="text-[10px] text-theme-text-muted truncate mt-0.5">
                  {{ song.artist || 'Unknown Artist' }}
                </div>
              </div>

              <!-- Duration -->
              <div class="text-[10px] font-mono text-theme-text-disabled pr-1 tabular-nums">
                {{ formatDuration(song.duration) }}
              </div>
            </div>
          </template>

          <!-- Empty State -->
          <div v-else class="flex flex-col items-center justify-center py-20 text-center">
            <svg-sprite src="Song" class="w-10 h-10 text-theme-border-hover mb-2" />
            <span class="text-xs font-semibold text-theme-text-disabled">No songs in queue</span>
          </div>
        </div>
      </overlay-scrollbars-component>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick, onMounted } from 'vue'
  import { storeToRefs } from 'pinia'
  import { invoke } from '@tauri-apps/api/core'
  import { useAudioPlayer } from '@groovex/state'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { formatDuration } from '@groovex/core'

  const player = useAudioPlayer()
  const { playlist, currentIndex, isPlaying, currentSong } = storeToRefs(player)

  const activeRowRef = ref<HTMLElement | null>(null)
  const queueListOsInstance = ref<any>(null)
  const dynamicAccentColor = ref('var(--color-theme-accent-glow)')

  // Edit states
  const isEditingLyrics = ref(false)
  const lyricsDraft = ref('')
  const isSavingLyrics = ref(false)

  const lyricLines = computed(() => {
    if (!currentSong.value?.lyrics) return []
    return currentSong.value.lyrics.split('\n')
  })

  function startEditLyrics() {
    lyricsDraft.value = currentSong.value?.lyrics || ''
    isEditingLyrics.value = true
  }

  function cancelEditLyrics() {
    isEditingLyrics.value = false
  }

  async function handleSaveLyrics() {
    if (!currentSong.value) return
    isSavingLyrics.value = true
    try {
      await invoke('update_song_lyrics', {
        songId: currentSong.value.id,
        lyrics: lyricsDraft.value,
      })
      player.updateLyrics(currentSong.value.id, lyricsDraft.value)
      isEditingLyrics.value = false
    } catch (err) {
      console.error('Failed to save lyrics:', err)
    } finally {
      isSavingLyrics.value = false
    }
  }

  // Reset editing mode when song changes
  watch(currentSong, () => {
    isEditingLyrics.value = false
    lyricsDraft.value = ''
  })

  // Track metadata computeds
  const songTitle = computed(() => currentSong.value?.title || currentSong.value?.filename || 'No song selected')
  const artistName = computed(() => currentSong.value?.artist || 'Unknown Artist')
  const thumbnailUrl = computed(
    () =>
      currentSong.value?.thumbnail ||
      'https://photo-resize-zmp3.zmdcdn.me/w240_r1x1_jpeg/cover/b/4/b/9/b4b96e865b138912ffc25bbb203f0c55.jpg',
  )

  function setActiveRowRef(el: any, idx: number) {
    if (idx === currentIndex.value) {
      activeRowRef.value = el
    }
  }

  function handlePlaySongAt(idx: number) {
    const song = playlist.value[idx]
    if (song) {
      player.playSong(song)
    }
  }

  function onQueueListOsInitialized(instance: any) {
    queueListOsInstance.value = instance
  }

  function scrollToActive() {
    if (queueListOsInstance.value && activeRowRef.value) {
      const { viewport } = queueListOsInstance.value.elements()
      const activeEl = activeRowRef.value

      // Calculate offset relative to the scrollable container using bounding rects
      const targetTop = activeEl.getBoundingClientRect().top - viewport.getBoundingClientRect().top + viewport.scrollTop - 80

      viewport.scrollTo({
        top: targetTop,
        behavior: 'smooth',
      })
    }
  }

  // Dominant average color extraction
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

          // Regulate brightness to keep background clean and text legible
          const brightness = (r * 299 + g * 587 + b * 114) / 1000
          if (brightness > 120) {
            const factor = 120 / brightness
            r = Math.round(r * factor)
            g = Math.round(g * factor)
            b = Math.round(b * factor)
          }
          resolve(`rgb(${r}, ${g}, ${b})`)
        } else {
          resolve('var(--color-theme-accent-glow)')
        }
      }
      img.onerror = () => resolve('var(--color-theme-accent-glow)')
      img.src = imageUrl
    })
  }

  // Auto scroll on mount and when references resolve
  onMounted(() => {
    nextTick(() => {
      scrollToActive()
    })
  })

  // Watch state changes to dynamically scroll active row
  watch(
    [currentIndex, activeRowRef],
    ([idx, activeEl]) => {
      if (idx !== -1 && activeEl) {
        nextTick(() => {
          scrollToActive()
        })
      }
    },
    { immediate: true },
  )

  // Dynamic page title update (SEO best practice)
  watch(
    currentSong,
    (song) => {
      if (song) {
        document.title = `${song.title || song.filename.replace(/\.[^/.]+$/, '')} - ${song.artist || 'Unknown Artist'} | GrooveX`
      } else {
        document.title = 'Now Playing | GrooveX'
      }
    },
    { immediate: true },
  )

  // Extract accent color whenever song cover changes
  watch(
    () => currentSong.value?.thumbnail,
    async (newVal) => {
      if (newVal) {
        try {
          const avgColor = await getAverageColor(newVal)
          dynamicAccentColor.value = avgColor
        } catch (e) {
          console.error('Failed to extract color:', e)
          dynamicAccentColor.value = 'var(--color-theme-accent-glow)'
        }
      } else {
        dynamicAccentColor.value = 'var(--color-theme-accent-glow)'
      }
    },
    { immediate: true },
  )
</script>

<style>
  .custom-scrollbar::-webkit-scrollbar {
    width: 4px;
  }
  .custom-scrollbar::-webkit-scrollbar-track {
    background: transparent;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb {
    background: rgba(255, 255, 255, 0.08);
    border-radius: 9999px;
  }
  .custom-scrollbar::-webkit-scrollbar-thumb:hover {
    background: rgba(255, 255, 255, 0.15);
  }
</style>
