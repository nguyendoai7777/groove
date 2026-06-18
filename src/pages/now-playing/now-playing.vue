<template>
  <div class="now-playing-page h-now-playing-h p-4 text-theme-text flex gap-5 relative select-none overflow-hidden">
    <!-- Immersive Dynamic Background Glow -->
    <div
      class="absolute top-0 left-0 right-0 h-now-playing-glow-h -z-10 bg-linear-to-b from-(--dynamic-accent) to-transparent opacity-30 pointer-events-none transition-all duration-700 ease-out"
      :style="{ '--dynamic-accent': dynamicAccentColor }"></div>

    <!-- Left side: Album Art & Controls Info -->
    <div
      class="w-1/4 min-w-now-playing-left-min-w max-w-now-playing-left-max-w flex flex-col items-center justify-start pt-4 shrink-0 @container h-full overflow-hidden">
      <!-- Album Cover Card -->
      <div
        class="relative group w-full aspect-square rounded-2xl overflow-hidden shadow-now-playing-cover bg-theme-bg-placeholder transition-all duration-500 hover:scale-[1.02] hover:shadow-now-playing-cover-hover shrink-0">
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
      <div class="text-center mt-4 w-full px-2 shrink-0">
        <h1 class="text-base font-bold truncate text-white mb-1" id="now-playing-title">
          {{ songTitle }}
        </h1>
        <p class="text-theme-text-muted text-xs truncate">{{ artistName }}</p>
      </div>

      <!-- Simple visualizer animation -->
      <div class="flex items-center gap-1.5 h-6 mt-3 shrink-0">
        <playing-visualizer :paused="!isPlaying" class="w-5 h-5 text-theme-accent-light" />
      </div>

      <!-- Timeline segments box -->
      <div
        v-if="parsedTimeline.length > 0"
        class="w-full flex-1 min-h-0 mt-4 flex flex-col overflow-hidden bg-theme-bg-item/20 border border-theme-border/40 rounded-xl p-3 text-left">
        <div
          class="text-[11px] font-bold text-theme-text-muted uppercase tracking-wider mb-2 border-b border-theme-border/30 pb-1.5 flex justify-between items-center shrink-0">
          <span>Timeline</span>
          <span class="text-[10px] text-theme-accent-light lowercase font-mono">
            {{ activeSegmentIndex !== -1 ? `${activeSegmentIndex + 1}/${parsedTimeline.length}` : '' }}
          </span>
        </div>
        <overlay-scrollbars-component :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="flex-1 pr-1 overflow-y-auto">
          <div class="flex flex-col gap-1">
            <div
              v-for="(seg, idx) in parsedTimeline"
              :key="idx"
              @click="player.seek(seg.start)"
              class="flex items-center gap-2 px-2 py-1.5 rounded-lg cursor-pointer transition-all duration-150 border border-transparent"
              :class="[
                idx === activeSegmentIndex
                  ? 'bg-theme-accent/10 border-theme-accent/30 text-theme-accent-light font-semibold shadow-xs'
                  : 'hover:bg-theme-bg-placeholder/25 text-theme-text-secondary',
              ]">
              <span class="font-mono text-[9px] shrink-0 opacity-70" :class="{ 'text-theme-accent-light': idx === activeSegmentIndex }">
                {{ formatDuration(seg.start) }}
              </span>
              <span class="truncate text-left flex-1 text-[11px]">{{ seg.title }}</span>
            </div>
          </div>
        </overlay-scrollbars-component>
      </div>
    </div>

    <!-- Middle side: Lyrics View & Editor -->
    <div class="flex-1 min-w-0 flex flex-col h-full backdrop-blur-xs overflow-hidden rounded-2xl border border-theme-border/50 py-2">
      <!-- View Mode -->
      <template v-if="!isEditingLyrics">
        <div class="flex items-center justify-between mb-3 border-b border-theme-border/40 pb-2.5 shrink-0 relative">
          <span class="text-2xl font-semibold tracking-wider absolute top-1/2 left-1/2 -translate-1/2">Lyrics</span>
          <div class="ml-auto flex gap-2">
            <!-- Timeline Edit Button -->
            <v-btn
              v-slot:default
              v-if="currentSong"
              @click="isEditingTimeline = true"
              class="flex rounded-full items-center gap-1.5 px-3 py-1 bg-theme-bg-placeholder/50 hover:bg-theme-border-hover/50 text-xs text-theme-accent-light font-semibold border border-theme-border-hover/30 hover:scale-[1.02] active:scale-95 text-none h-auto shadow-none cursor-pointer"
              variant="flat">
              <div class="flex items-center gap-1.5 py-0.5">
                <svg-sprite src="Loop" class="w-3.5 h-3.5" />
                <span>Timeline</span>
              </div>
            </v-btn>
            <!-- Edit Lyrics Button -->
            <v-btn
              v-slot:default
              v-if="currentSong"
              @click="startEditLyrics"
              class="flex rounded-full items-center gap-1.5 px-3 py-1 bg-theme-bg-placeholder/50 hover:bg-theme-border-hover/50 text-xs text-theme-accent-light font-semibold border border-theme-border-hover/30 hover:scale-[1.02] active:scale-95 text-none h-auto shadow-none cursor-pointer"
              variant="flat">
              <div class="flex items-center gap-1.5 py-0.5">
                <svg-sprite src="Edit" class="w-3.5 h-3.5" />
                <span>{{ lyricLines.length > 0 ? 'Edit Lyrics' : 'Add Lyrics' }}</span>
              </div>
            </v-btn>
          </div>
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

    <!-- Timeline Edit Dialog -->
    <v-dialog v-model="isEditingTimeline" max-width="500px">
      <v-card class="bg-theme-bg-card border border-theme-border/60 text-theme-text rounded-2xl overflow-hidden p-4 shadow-xl">
        <v-card-title class="px-2 pb-2 text-base font-bold text-white border-b border-theme-border/30">Edit Timeline</v-card-title>
        <v-card-text class="px-2 py-4">
          <p class="text-xs text-theme-text-muted mb-3 leading-relaxed text-left">
            Format:
            <strong>[start_time] [space] song name</strong>
            (one per line) .
            <br />
            Time can be in raw seconds, MM:SS, or HH:MM:SS. Example:
            <br />
            <span class="font-mono text-theme-accent-light">
              0:00 SANYO
              <br />
              0:25 Nu Sieu Anh Hung
            </span>
          </p>
          <textarea
            v-model="timelineDraft"
            placeholder="e.g. 0:00 First Track"
            rows="10"
            class="w-full bg-theme-bg-item/40 border border-theme-border focus:border-theme-accent/50 rounded-xl p-3 text-xs text-theme-text-secondary outline-hidden resize-none font-mono leading-relaxed custom-scrollbar transition-colors focus:ring-1 focus:ring-theme-accent/20"></textarea>
        </v-card-text>
        <v-card-actions class="px-2 pt-2 border-t border-theme-border/30 justify-end gap-2">
          <custom-btn variant="secondary" @click="isEditingTimeline = false" :disabled="isSavingTimeline">Cancel</custom-btn>
          <custom-btn variant="primary" @click="handleSaveTimeline" :loading="isSavingTimeline">Save</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
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
  const { playlist, currentIndex, isPlaying, currentSong, currentTime, duration } = storeToRefs(player)

  const activeRowRef = ref<HTMLElement | null>(null)
  const queueListOsInstance = ref<any>(null)
  const dynamicAccentColor = ref('var(--color-theme-accent-glow)')

  // Edit states (lyrics & timeline)
  const isEditingLyrics = ref(false)
  const lyricsDraft = ref('')
  const isSavingLyrics = ref(false)

  const isEditingTimeline = ref(false)
  const timelineDraft = ref('')
  const isSavingTimeline = ref(false)

  // Timeline Interface
  interface TimelineSegment {
    start: number
    end: number
    title: string
  }

  // Convert time string "MM:SS", "HH:MM:SS" or raw seconds to number
  function parseTimeToSeconds(timeStr: string): number {
    const parts = timeStr.split(':').map(Number)
    if (parts.some(isNaN)) return 0

    if (parts.length === 1) {
      return parts[0]
    } else if (parts.length === 2) {
      return parts[0] * 60 + parts[1]
    } else if (parts.length === 3) {
      return parts[0] * 3600 + parts[1] * 60 + parts[2]
    }
    return 0
  }

  // Parse raw timeline text:
  // e.g. "0:00 SANYO"
  // e.g. "0:25 Nu Sieu Anh Hung"
  const parsedTimeline = computed<TimelineSegment[]>(() => {
    if (!currentSong.value?.timeline) return []
    const lines = currentSong.value.timeline.split('\n')
    const rawSegments: { start: number; title: string }[] = []

    for (const line of lines) {
      const trimmed = line.trim()
      if (!trimmed) continue

      const match = trimmed.match(/^([\d:]+)\s+(.+)$/)
      if (match) {
        const startStr = match[1]
        const title = match[2].trim()
        const start = parseTimeToSeconds(startStr)
        rawSegments.push({ start, title })
      }
    }

    rawSegments.sort((a, b) => a.start - b.start)

    const segments: TimelineSegment[] = []
    const totalDuration = duration.value || 0

    for (let i = 0; i < rawSegments.length; i++) {
      const start = rawSegments[i].start
      const title = rawSegments[i].title
      const nextStart = i < rawSegments.length - 1 ? rawSegments[i + 1].start : totalDuration
      const end = Math.max(start, nextStart)
      segments.push({ start, end, title })
    }

    return segments
  })

  // Active segment index
  const activeSegmentIndex = computed(() => {
    const time = currentTime.value
    return parsedTimeline.value.findIndex((seg, idx) => {
      const isLast = idx === parsedTimeline.value.length - 1
      return time >= seg.start && (isLast ? time <= seg.end : time < seg.end)
    })
  })

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

  async function handleSaveTimeline() {
    if (!currentSong.value) return
    isSavingTimeline.value = true
    try {
      await invoke('update_song_timeline', {
        songId: currentSong.value.id,
        timeline: timelineDraft.value,
      })
      player.updateTimeline(currentSong.value.id, timelineDraft.value)
      isEditingTimeline.value = false
    } catch (err) {
      console.error('Failed to save timeline:', err)
    } finally {
      isSavingTimeline.value = false
    }
  }

  // Reset editing mode when song changes
  watch(currentSong, () => {
    isEditingLyrics.value = false
    isEditingTimeline.value = false
    lyricsDraft.value = ''
    timelineDraft.value = currentSong.value?.timeline || ''
  })

  // Pre-populate timeline draft when modal opens
  watch(isEditingTimeline, (open) => {
    if (open) {
      timelineDraft.value = currentSong.value?.timeline || ''
    }
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
