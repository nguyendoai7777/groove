<template>
  <div
    class="now-playing-page h-[calc(100vh-var(--nav-head-h)-var(--audio-controller-h)-24px)] p-4 text-zinc-100 flex gap-5 relative select-none overflow-hidden">
    <!-- Immersive Dynamic Background Glow -->
    <div
      class="absolute top-0 left-0 right-0 h-[450px] -z-10 bg-linear-to-b from-(--dynamic-accent) to-transparent opacity-30 pointer-events-none transition-all duration-700 ease-out"
      :style="{ '--dynamic-accent': dynamicAccentColor }"></div>

    <!-- Left side: Album Art & Controls Info -->
    <div class="w-[25%] min-w-[180px] max-w-[300px] flex flex-col items-center justify-start pt-4 shrink-0 @container">
      <!-- Album Cover Card -->
      <div
        class="relative group w-full aspect-square rounded-2xl overflow-hidden shadow-[0_20px_50px_rgba(0,0,0,0.5)] bg-zinc-800 transition-all duration-500 hover:scale-[1.02] hover:shadow-[0_25px_60px_rgba(0,0,0,0.6)]">
        <img
          v-if="thumbnailUrl"
          :src="thumbnailUrl"
          alt="Now Playing Cover"
          class="w-full h-full object-cover transition-transform duration-700 group-hover:scale-105" />
        <div v-else class="w-full h-full flex items-center justify-center bg-linear-to-br from-zinc-700 to-zinc-900">
          <svg-sprite src="Album" class="w-16 h-16 text-zinc-500" />
        </div>
      </div>

      <!-- Metadata -->
      <div class="text-center mt-5 w-full px-2">
        <h1 class="text-base font-bold truncate text-white mb-1" id="now-playing-title">
          {{ songTitle }}
        </h1>
        <p class="text-zinc-400 text-xs truncate">{{ artistName }}</p>
      </div>

      <!-- Simple visualizer animation -->
      <div class="flex items-center gap-1.5 h-6 mt-4">
        <playing-visualizer :paused="!isPlaying" class="w-5 h-5 text-cyan-400" />
      </div>
    </div>

    <!-- Middle side: Lyrics View & Editor -->
    <div
      class="flex-1 min-w-0 flex flex-col h-full bg-zinc-900/10 backdrop-blur-xs p-4 rounded-2xl border border-zinc-800/20 overflow-hidden">
      <!-- View Mode -->
      <template v-if="!isEditingLyrics">
        <div class="flex items-center justify-between mb-3 border-b border-zinc-800/40 pb-2.5 shrink-0">
          <span class="text-xs text-zinc-500 font-semibold tracking-wider uppercase">Lyrics</span>
          <v-btn
            v-if="currentSong"
            @click="startEditLyrics"
            class="flex items-center gap-1.5 px-3 py-1 bg-zinc-800/50 hover:bg-zinc-700/50 text-xs text-cyan-400 font-semibold border border-zinc-700/30 hover:scale-[1.02] active:scale-95 text-none h-auto shadow-none cursor-pointer"
            variant="flat">
            <div class="flex items-center gap-1.5 py-0.5">
              <svg-sprite src="Edit" class="w-3.5 h-3.5" />
              <span>{{ lyricLines.length > 0 ? 'Edit Lyrics' : 'Add Lyrics' }}</span>
            </div>
          </v-btn>
        </div>

        <!-- No song loaded state -->
        <div v-if="!currentSong" class="flex flex-col items-center justify-center py-20 text-center flex-1">
          <svg-sprite src="HelpCircle" class="w-10 h-10 text-zinc-700 mb-3" />
          <span class="text-xs font-semibold text-zinc-500">Play a song to view lyrics</span>
        </div>

        <!-- Lyrics content -->
        <overlay-scrollbars-component
          v-else-if="lyricLines.length > 0"
          :options="{ scrollbars: { autoHide: 'scroll' } }"
          defer
          class="flex-1 pr-1 text-center py-2">
          <div class="flex flex-col gap-1.5">
            <p
              v-for="(line, idx) in lyricLines"
              :key="idx"
              class="text-xs md:text-sm font-semibold py-0.5 text-zinc-300 hover:text-white transition-colors duration-150"
              :class="{ 'h-4': line.trim() === '' }">
              {{ line }}
            </p>
          </div>
        </overlay-scrollbars-component>

        <!-- Empty State -->
        <div v-else class="flex flex-col items-center justify-center py-20 text-center flex-1">
          <svg-sprite src="LyricsBubble" class="w-10 h-10 text-zinc-700 mb-3" />
          <span class="text-xs font-semibold text-zinc-400">No lyrics available for this song</span>
        </div>
      </template>

      <!-- Edit Mode -->
      <template v-else>
        <div class="flex flex-col gap-3 flex-1 h-full">
          <div class="flex items-center justify-between border-b border-zinc-800/40 pb-2.5 shrink-0">
            <span class="text-xs text-zinc-400 font-semibold uppercase tracking-wider">
              {{ currentSong?.lyrics ? 'Edit Lyrics' : 'Add Lyrics' }}
            </span>
            <span class="text-[10px] text-zinc-500">Lưu vào file & database</span>
          </div>

          <!-- Textarea -->
          <textarea
            v-model="lyricsDraft"
            placeholder="Paste or write lyrics here..."
            class="w-full flex-1 bg-zinc-950/40 border border-zinc-800 focus:border-cyan-500/50 rounded-xl p-3 text-xs text-zinc-200 outline-hidden resize-none font-sans leading-relaxed custom-scrollbar transition-colors focus:ring-1 focus:ring-cyan-500/20"></textarea>

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
      class="w-[32%] min-w-[240px] max-w-[340px] flex flex-col h-full bg-zinc-900/10 backdrop-blur-xs p-4 rounded-2xl border border-zinc-800/20 overflow-hidden">
      <div class="flex items-center justify-between mb-3 pb-2 border-b border-zinc-800/30 shrink-0">
        <h3 class="text-sm font-bold text-white tracking-wide flex items-center gap-2">
          <span>Queue / Playlist</span>
          <span class="text-[10px] font-normal text-zinc-500">({{ playlist.length }} tracks)</span>
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
                idx === currentIndex ? 'bg-cyan-500/10 border-cyan-500/30 shadow-md text-white' : 'hover:bg-zinc-800/40 text-zinc-300',
              ]"
              @click="handlePlaySongAt(idx)">
              <!-- Index / Status -->
              <div class="w-6 flex items-center justify-center text-[10px] font-mono text-zinc-500">
                <span v-if="idx !== currentIndex" class="group-hover/item:hidden">{{ idx + 1 }}</span>
                <svg-sprite
                  v-if="idx !== currentIndex"
                  src="Play"
                  class="w-2.5 h-2.5 text-cyan-400 fill-cyan-400 hidden group-hover/item:block" />
                <playing-visualizer v-if="idx === currentIndex" :paused="!isPlaying" class="w-3 h-3 text-cyan-400" />
              </div>

              <!-- Thumbnail -->
              <div class="w-8 h-8 rounded-md overflow-hidden bg-zinc-800 shrink-0 shadow-xs">
                <img v-if="song.thumbnail" :src="song.thumbnail" class="w-full h-full object-cover" />
                <div v-else class="w-full h-full flex items-center justify-center bg-zinc-900">
                  <svg-sprite src="Album" class="w-4 h-4 text-zinc-600" />
                </div>
              </div>

              <!-- Title & Artist -->
              <div class="flex-1 min-w-0 text-left">
                <div
                  class="text-xs font-semibold truncate"
                  :class="idx === currentIndex ? 'text-cyan-400' : 'text-zinc-200 group-hover/item:text-white'">
                  {{ song.title || song.filename.replace(/\.[^/.]+$/, '') }}
                </div>
                <div class="text-[10px] text-zinc-400 truncate mt-0.5">
                  {{ song.artist || 'Unknown Artist' }}
                </div>
              </div>

              <!-- Duration -->
              <div class="text-[10px] font-mono text-zinc-500 pr-1 tabular-nums">
                {{ formatDuration(song.duration) }}
              </div>
            </div>
          </template>

          <!-- Empty State -->
          <div v-else class="flex flex-col items-center justify-center py-20 text-center">
            <svg-sprite src="Song" class="w-10 h-10 text-zinc-700 mb-2" />
            <span class="text-xs font-semibold text-zinc-500">No songs in queue</span>
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
  const dynamicAccentColor = ref('rgba(6, 182, 212, 0.4)')

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
          resolve('#06b6d4')
        }
      }
      img.onerror = () => resolve('#06b6d4')
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
          dynamicAccentColor.value = 'rgba(6, 182, 212, 0.4)'
        }
      } else {
        dynamicAccentColor.value = 'rgba(6, 182, 212, 0.4)'
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
