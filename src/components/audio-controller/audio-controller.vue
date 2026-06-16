<template>
  <div
    class="AudioController fixed bottom-0 left-0 w-full z-50 border-t border-zinc-800 bg-[#121212] pr-6 flex items-center justify-between text-white select-none"
  >
    <!-- Left Section: Song Info -->
    <div class="flex items-center gap-3 w-[25%] min-w-60">
      <!-- Album Cover with stats and hover overlay -->
      <div
        class="relative overflow-hidden group shrink-0 cursor-pointer shadow-md aspect-square"
        style="height: var(--audio-controller-h)"
      >
        <img
          :src="thumbnailUrl"
          class="transition-transform duration-300 group-hover:scale-105"
          alt="Song Cover"
        />
        <div
          class="absolute inset-0 bg-black/40 opacity-0 group-hover:opacity-100 flex items-center justify-center transition-opacity duration-200"
        >
          <toggle-play :isPlaying="isPlaying" @state="onPlayStateChange" />
        </div>
        <!-- Stats badge -->
      </div>

      <!-- Metadata (Artist, Title, Sub-artists) -->
      <div class="flex flex-col min-w-0 pr-2 leading-tight">
        <span
          class="text-[11px] text-zinc-400 truncate hover:text-white cursor-pointer transition-colors"
          >{{ artistName }}</span
        >
        <div
          ref="titleContainerRef"
          class="song-title-container overflow-hidden whitespace-nowrap my-1"
        >
          <div
            class="song-title-content inline-flex cursor-pointer text-white"
            :class="{ 'animate-marquee': isTitleOverflow }"
          >
            <span
              ref="titleTextRef"
              class="text-[13px] font-semibold transition-colors duration-200"
              :class="{ 'pr-8': isTitleOverflow }"
            >
              <span ref="titleInnerRef">{{ songTitle }}</span>
            </span>
            <span
              v-if="isTitleOverflow"
              class="text-[13px] font-semibold pr-8 transition-colors duration-200"
              aria-hidden="true"
            >
              {{ songTitle }}
            </span>
          </div>
        </div>
        <span
          class="text-[10px] text-zinc-500 truncate hover:text-zinc-300 cursor-pointer transition-colors"
          >{{ subArtists }}</span
        >
      </div>
    </div>

    <!-- Center Section: Playback Controls & Progress Bar -->
    <div class="flex flex-col items-center flex-1 h-full">
      <!-- Controls -->
      <div class="flex items-center gap-4 mb-1">
        <!-- Shuffle -->
        <icon-btn
          src="Shuffle"
          class="transition-colors relative group rounded-full"
          :class="isShuffle ? 'text-green-500' : 'text-zinc-400 hover:text-white'"
          @click="toggleShuffle"
          title="Shuffle"
        />

        <!-- Previous -->
        <icon-btn
          src="Prev"
          class="text-zinc-400 hover:text-white rounded-full transition-colors"
          @click="prevTrack"
          title="Previous"
        />

        <!-- Play/Pause (animated toggle-play) -->
        <toggle-play :is-playing="isPlaying" @state="onPlayStateChange" style="--size: 36px" />

        <!-- Next -->
        <icon-btn
          src="Next"
          class="text-zinc-400 hover:text-white rounded-full transition-colors"
          @click="nextTrack"
          title="Next"
        />

        <!-- Repeat -->
        <icon-btn
          src="Loop"
          class="transition-colors relative group rounded-full"
          :class="isRepeat ? 'text-green-500' : 'text-zinc-400 hover:text-white'"
          @click="toggleRepeat"
          title="Repeat"
        />
      </div>

      <!-- Timeline/Progress -->
      <div class="w-full flex items-center gap-3 text-[11px] text-zinc-400 leading-none">
        <span class="w-9 text-right font-mono">{{ formatTime(currentTime) }}</span>

        <div class="flex-1 grx-PlayerSlider flex items-center h-4">
          <v-slider
            :model-value="currentTime"
            @update:model-value="player.seek"
            :min="0"
            :max="duration"
            :step="1"
            :track-size="4"
            :thumb-size="16"
            hide-details
            density="compact"
            class="w-full cursor-pointer"
          />
        </div>

        <span class="w-9 text-left font-mono">{{ formatTime(duration) }}</span>
      </div>
    </div>

    <!-- Right Section: Volume & Options -->
    <div class="flex items-center gap-3 w-[20%] justify-end min-w-32.5 pl-20">
      <!-- Volume Toggle Button -->
      <icon-btn
        :src="volumeIcon"
        class="text-zinc-400 hover:text-white rounded-full transition-colors"
        @click="toggleMute"
        :title="isMuted ? 'Unmute' : 'Mute'"
      />

      <!-- Volume Slider -->
      <div class="w-24 grx-VolumeSlider flex items-center h-4">
        <v-slider
          v-model="volume"
          :min="0"
          :max="100"
          :step="1"
          :track-size="2"
          :thumb-size="8"
          hide-details
          density="compact"
          class="w-full cursor-pointer"
        />
      </div>

      <!-- Far Right Menu Button -->
      <icon-btn
        src="Settings"
        :size="{ box: 'sm', icon: 'sm' }"
        class="text-zinc-400 hover:text-white rounded-full transition-colors ml-1"
        title="Settings"
      />
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed, ref, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
  import { storeToRefs } from 'pinia'
  import { useAudioPlayer } from '@groovex/state'
  import IconBtn from '@groovex/ui/button/icon-btn.vue'
  import TogglePlay from './toggle-play.vue'

  const player = useAudioPlayer()
  const { currentSong, isPlaying, currentTime, duration, volume, isMuted, isShuffle, isRepeat } =
    storeToRefs(player)
  const { toggleShuffle, toggleRepeat } = player

  const titleContainerRef = ref<HTMLElement | null>(null)
  const titleInnerRef = ref<HTMLElement | null>(null)
  const isTitleOverflow = ref(false)
  // Track metadata computed properties
  const songTitle = computed(
    () => currentSong.value?.title || currentSong.value?.filename || 'No song selected',
  )
  const artistName = computed(() => currentSong.value?.artist || 'Unknown Artist')
  const subArtists = computed(() => '') // Song model doesn't have sub-artists
  const thumbnailUrl = computed(
    () =>
      currentSong.value?.thumbnail ||
      'https://photo-resize-zmp3.zmdcdn.me/w240_r1x1_jpeg/cover/b/4/b/9/b4b96e865b138912ffc25bbb203f0c55.jpg',
  )

  function checkOverflow() {
    if (titleContainerRef.value && titleInnerRef.value) {
      isTitleOverflow.value = titleInnerRef.value.offsetWidth > titleContainerRef.value.clientWidth
    }
  }

  watch(songTitle, () => {
    isTitleOverflow.value = false
    nextTick(() => {
      checkOverflow()
    })
  })

  let resizeObserver: ResizeObserver | null = null

  onMounted(() => {
    checkOverflow()
    if (typeof ResizeObserver !== 'undefined' && titleContainerRef.value) {
      resizeObserver = new ResizeObserver(() => {
        checkOverflow()
      })
      resizeObserver.observe(titleContainerRef.value)
    }
  })

  onBeforeUnmount(() => {
    if (resizeObserver) {
      resizeObserver.disconnect()
    }
  })

  // Time formatter
  function formatTime(seconds: number): string {
    const m = Math.floor(seconds / 60)
    const s = Math.floor(seconds % 60)
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}`
  }

  function onPlayStateChange(state: boolean) {
    player.setPlaying(state)
  }

  // Volume control & dynamic icon mapping
  const volumeIcon = computed(() => {
    if (isMuted.value || volume.value === 0) {
      return 'VMute'
    }
    if (volume.value >= 1 && volume.value <= 35) {
      return 'VMin'
    }
    if (volume.value >= 36 && volume.value <= 65) {
      return 'VMedium'
    }
    return 'VMax'
  })

  function toggleMute() {
    isMuted.value = !isMuted.value
  }

  function prevTrack() {
    player.prevTrack()
  }

  function nextTrack() {
    player.nextTrack()
  }
</script>

<style>
  .AudioController {
    height: var(--audio-controller-h);
    background-color: var(--audio-controller-bg);
  }

  .song-title-container {
    width: 100%;
  }

  .song-title-content {
    --song-hover-color: #3b82f6; /* Hover color variable. Adjust/override this later as needed. */
    transition: color 0.2s ease;
  }

  .song-title-content:hover {
    color: var(--song-hover-color) !important;
  }

  .animate-marquee {
    animation: marquee 12s linear infinite;
  }

  .animate-marquee:hover {
    animation-play-state: paused;
  }

  @keyframes marquee {
    0% {
      transform: translate3d(0, 0, 0);
    }
    100% {
      transform: translate3d(-50%, 0, 0);
    }
  }
</style>
