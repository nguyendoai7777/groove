<template>
  <div
    class="now-playing-page min-h-full p-6 text-zinc-100 flex flex-col md:flex-row gap-8 relative select-none"
  >
    <!-- Immersive Dynamic Background Glow -->
    <div
      class="absolute top-0 left-0 right-0 h-[450px] -z-10 bg-gradient-to-b from-[var(--dynamic-accent)] to-transparent opacity-30 pointer-events-none transition-all duration-700 ease-out"
      :style="{ '--dynamic-accent': dynamicAccentColor }"
    ></div>

    <!-- Left side: Album Art & Controls Info -->
    <div
      class="flex-1 flex flex-col items-center justify-center md:sticky md:top-24 h-fit max-w-md mx-auto"
    >
      <!-- Album Cover Card -->
      <div
        class="relative group aspect-square w-72 md:w-80 rounded-2xl overflow-hidden shadow-[0_20px_50px_rgba(0,0,0,0.5)] bg-zinc-800 transition-all duration-500 hover:scale-[1.03] hover:shadow-[0_25px_60px_rgba(0,0,0,0.6)]"
      >
        <img
          v-if="thumbnailUrl"
          :src="thumbnailUrl"
          alt="Now Playing Cover"
          class="w-full h-full object-cover transition-transform duration-700 group-hover:scale-105"
        />
        <div
          v-else
          class="w-full h-full flex items-center justify-center bg-gradient-to-br from-zinc-700 to-zinc-900"
        >
          <svg-sprite src="Album" class="w-24 h-24 text-zinc-500" />
        </div>
      </div>

      <!-- Metadata -->
      <div class="text-center mt-6 w-full px-4">
        <h1 class="text-2xl font-bold truncate text-white mb-2" id="now-playing-title">
          {{ songTitle }}
        </h1>
        <p class="text-zinc-400 text-sm truncate">{{ artistName }}</p>
      </div>

      <!-- Simple visualizer animation -->
      <div class="flex items-center gap-1.5 h-8 mt-6">
        <playing-visualizer :paused="!isPlaying" class="w-6 h-6 text-cyan-400" />
      </div>
    </div>

    <!-- Right side: Playlist Queue List -->
    <div class="flex-1 min-w-0 md:max-w-xl flex flex-col">
      <div class="flex items-center justify-between mb-4">
        <h3 class="text-lg font-bold text-white tracking-wide flex items-center gap-2">
          <span>Queue / Playlist</span>
          <span class="text-xs font-normal text-zinc-500">({{ playlist.length }} tracks)</span>
        </h3>
      </div>

      <!-- Queue Container -->
      <div
        class="flex flex-col gap-2 bg-zinc-900/20 backdrop-blur-xs p-4 rounded-2xl border border-zinc-800/30"
      >
        <!-- Render playlist -->
        <template v-if="playlist.length > 0">
          <div
            v-for="(song, idx) in playlist"
            :key="song.id + '-' + idx"
            :id="'queue-item-' + song.id"
            :ref="(el) => setActiveRowRef(el, idx)"
            class="group/item flex items-center gap-3 p-3 rounded-xl transition-all duration-300 cursor-pointer border border-transparent"
            :class="[
              idx < currentIndex ? 'opacity-25 hover:opacity-50' : 'opacity-100',
              idx === currentIndex
                ? 'bg-cyan-500/10 border-cyan-500/30 shadow-md text-white'
                : 'hover:bg-zinc-800/40 text-zinc-300',
            ]"
            @click="handlePlaySongAt(idx)"
          >
            <!-- Index / Status -->
            <div class="w-8 flex items-center justify-center text-xs font-mono text-zinc-500">
              <span v-if="idx !== currentIndex" class="group-hover/item:hidden">{{ idx + 1 }}</span>
              <svg-sprite
                v-if="idx !== currentIndex"
                src="Play"
                class="w-3 h-3 text-cyan-400 fill-cyan-400 hidden group-hover/item:block"
              />
              <playing-visualizer
                v-if="idx === currentIndex"
                :paused="!isPlaying"
                class="w-3.5 h-3.5 text-cyan-400"
              />
            </div>

            <!-- Thumbnail -->
            <div class="w-10 h-10 rounded-md overflow-hidden bg-zinc-800 shrink-0 shadow-sm">
              <img v-if="song.thumbnail" :src="song.thumbnail" class="w-full h-full object-cover" />
              <div v-else class="w-full h-full flex items-center justify-center bg-zinc-900">
                <svg-sprite src="Album" class="w-5 h-5 text-zinc-600" />
              </div>
            </div>

            <!-- Title & Artist -->
            <div class="flex-1 min-w-0">
              <div
                class="text-sm font-semibold truncate"
                :class="
                  idx === currentIndex
                    ? 'text-cyan-400'
                    : 'text-zinc-200 group-hover/item:text-white'
                "
              >
                {{ song.title || song.filename.replace(/\.[^/.]+$/, '') }}
              </div>
              <div class="text-xs text-zinc-400 truncate mt-0.5">
                {{ song.artist || 'Unknown Artist' }}
              </div>
            </div>

            <!-- Duration -->
            <div class="text-xs font-mono text-zinc-500 pr-1 tabular-nums">
              {{ formatDuration(song.duration) }}
            </div>
          </div>
        </template>

        <!-- Empty State -->
        <div v-else class="flex flex-col items-center justify-center py-20 text-center">
          <svg-sprite src="Song" class="w-12 h-12 text-zinc-700 mb-3" />
          <span class="text-sm font-semibold text-zinc-500">No songs in queue</span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick, onMounted } from 'vue'
  import { storeToRefs } from 'pinia'
  import { useAudioPlayer } from '@groovex/state'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import { useLayoutScroll } from '../../shared/composables/use-layout-scroll'
  import { formatDuration } from '@groovex/core'

  const player = useAudioPlayer()
  const { playlist, currentIndex, isPlaying, currentSong } = storeToRefs(player)
  const { osInstance } = useLayoutScroll()

  const activeRowRef = ref<HTMLElement | null>(null)
  const dynamicAccentColor = ref('rgba(6, 182, 212, 0.4)')

  // Track metadata computeds
  const songTitle = computed(
    () => currentSong.value?.title || currentSong.value?.filename || 'No song selected',
  )
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

  function scrollToActive() {
    if (osInstance.value && activeRowRef.value) {
      const viewport = osInstance.value.elements().viewport
      const activeEl = activeRowRef.value

      // Calculate relative offset and position the element 100px from the top of the container
      const targetTop =
        activeEl.getBoundingClientRect().top -
        viewport.getBoundingClientRect().top +
        viewport.scrollTop -
        100

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
    [osInstance, currentIndex, activeRowRef],
    ([instance, idx, activeEl]) => {
      if (instance && idx !== -1 && activeEl) {
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
