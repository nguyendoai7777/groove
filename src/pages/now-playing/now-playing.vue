<template>
  <div class="now-playing-page h-now-playing-h p-4 text-theme-text flex gap-5 relative select-none overflow-hidden">
    <!-- Immersive Dynamic Background Glow -->
    <div
      class="absolute top-0 left-0 right-0 h-now-playing-glow-h -z-10 bg-linear-to-b from-(--dynamic-accent) to-transparent opacity-30 pointer-events-none transition-all duration-700 ease-out"
      :style="{ '--dynamic-accent': dynamicAccentColor }"></div>

    <!-- Left side: Album Art & Controls Info -->
    <div
      class="w-1/4 min-w-now-playing-left-min-w max-w-now-playing-left-max-w flex flex-col items-center justify-start shrink-0 @container h-full overflow-hidden">
      <!-- Album Cover Card -->
      <div
        class="relative group w-full aspect-square rounded-2xl overflow-hidden shadow-now-playing-cover bg-theme-bg-placeholder transition-all duration-500 hover:shadow-now-playing-cover-hover shrink-0">
        <img
          v-if="thumbnailUrl"
          :src="thumbnailUrl"
          alt="Now Playing Cover"
          class="w-full h-full object-cover transition-transform duration-350" />
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

      <!-- Timeline segments box -->
      <div
        v-if="parsedTimeline.length > 0"
        class="w-full flex-1 min-h-0 mt-4 flex flex-col overflow-hidden bg-theme-bg-item/20 border border-theme-border/40 rounded-xl text-left">
        <div
          class="text-[11px] px-3 py-2 font-bold text-theme-text-muted tracking-wider mb-2 border-b border-theme-border/30 pb-1.5 flex justify-between items-center shrink-0">
          <!-- Simple visualizer animation -->
          <div class="flex items-center gap-1.5 shrink-0">
            <playing-visualizer :paused="!isPlaying" class="w-5 h-5 text-theme-accent-light" />
          </div>
          <span class="text-[10px] text-theme-accent-light lowercase font-mono">
            {{ activeSegmentIndex !== -1 ? `${activeSegmentIndex + 1}/${parsedTimeline.length}` : '' }}
          </span>
        </div>
        <overlay-scrollbars-component
          :options="{ scrollbars: { autoHide: 'scroll' } }"
          defer
          class="flex-1 px-2 overflow-y-auto timeline-scroll-container"
          @os-initialized="onTimelineOsInitialized">
          <div class="flex flex-col gap-1">
            <div
              v-for="(seg, idx) in parsedTimeline"
              class="flex items-center gap-2 px-2 py-1.5 rounded-lg cursor-pointer transition-all duration-150 border border-transparent snap-start"
              :key="idx"
              :ref="(el) => setActiveTimelineRowRef(el, idx)"
              :class="[
                idx === activeSegmentIndex
                  ? 'bg-theme-accent/10 border-theme-accent/30 text-theme-accent-light font-semibold shadow-xs'
                  : 'hover:bg-theme-bg-placeholder/25 text-theme-text-secondary',
              ]"
              @click="player.seek(seg.start)">
              <span
                class="font-mono text-[12px] shrink-0 opacity-70 mt-px"
                :class="{ 'text-theme-accent-light': idx === activeSegmentIndex }">
                {{ formatDuration(seg.start) }}
              </span>
              <span class="truncate text-left flex-1 text-[11px]">{{ seg.title }}</span>
            </div>
          </div>
        </overlay-scrollbars-component>
      </div>
    </div>

    <!-- Middle side: Lyrics View & Editor -->
    <div class="flex-1 min-w-0 flex flex-col h-full backdrop-blur-xs overflow-hidden rounded-2xl border border-theme-border/50 pb-2">
      <!-- View Mode -->
      <template v-if="!isEditingLyrics">
        <div class="flex items-center justify-between mb-3 border-b border-theme-border/40 p-2.5 shrink-0 relative">
          <span class="text-2xl font-semibold tracking-wider absolute top-1/2 left-1/2 -translate-1/2">Lyrics</span>
          <div class="ml-auto flex gap-2">
            <!-- Edit Metadata Button -->
            <v-btn
              v-slot:default
              v-if="currentSong"
              @click="startEditMetadata"
              class="flex rounded-full items-center gap-1.5 px-3 py-1 bg-theme-bg-placeholder/50 hover:bg-theme-border-hover/50 text-xs text-theme-accent-light font-semibold border border-theme-border-hover/30 hover:scale-[1.02] active:scale-95 text-none h-auto shadow-none cursor-pointer"
              variant="flat">
              <div class="flex items-center gap-1.5 py-0.5">
                <svg-sprite src="Album" class="w-3.5 h-3.5" />
                <span>Metadata</span>
              </div>
            </v-btn>
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
        <div class="flex flex-col flex-1 h-full">
          <div class="flex items-center justify-between border-b border-theme-border/40 py-2.5 px-2 shrink-0">
            <span class="text-xs text-theme-text-muted font-semibold tracking-wider">
              {{ currentSong?.lyrics ? 'Edit Lyrics' : 'Add Lyrics' }}
            </span>
            <span class="text-[10px] text-theme-text-disabled">Lưu vào file & database</span>
          </div>

          <!-- Contenteditable wrapped in OverlayScrollbars -->
          <overlay-scrollbars-component
            :options="{ scrollbars: { autoHide: 'scroll' } }"
            defer
            class="w-full flex-1 bg-theme-bg-item/40 border border-theme-border focus-within:border-theme-accent/50 transition-colors focus-within:ring-1 focus-within:ring-theme-accent/20">
            <div
              ref="lyricsEditorRef"
              contenteditable="true"
              placeholder="Paste or write lyrics here..."
              class="w-full min-h-full p-3 text-xs text-theme-text-secondary outline-hidden font-sans leading-relaxed whitespace-pre-wrap wrap-break-word empty:before:content-[attr(placeholder)] empty:before:text-theme-text-disabled/60 empty:before:pointer-events-none"
              @input="handleLyricsInput"></div>
          </overlay-scrollbars-component>

          <!-- Action buttons -->
          <div class="flex items-center justify-end gap-3 mt-3 shrink-0">
            <custom-btn variant="secondary" @click="cancelEditLyrics" :disabled="isSavingLyrics">Cancel</custom-btn>
            <custom-btn variant="primary" @click="handleSaveLyrics" :loading="isSavingLyrics">Save</custom-btn>
          </div>
        </div>
      </template>
    </div>

    <!-- Right side: Playlist Queue List -->
    <playlist-history class="shrink-0" />

    <!-- Metadata Edit Dialog -->
    <v-dialog v-model="isEditingMetadata" max-width="800">
      <v-card
        class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
        <v-card-title class="px-4 py-3 text-base font-bold border-b border-theme-border/30 bg-theme-bg-placeholder/20">
          Edit Metadata
        </v-card-title>
        <v-card-text class="px-1">
          <overlay-scrollbars-component :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="p-3 max-h-[70svh]">
            <div class="flex gap-4 items-start">
              <!-- Thumbnail cover selection -->
              <div class="flex flex-col gap-4 items-center">
                <div
                  class="relative w-[200px] h-[200px] rounded-lg overflow-hidden bg-theme-bg-placeholder border border-theme-border/50 shrink-0 group"
                  @mouseenter="isHoveringUpload = true"
                  @mouseleave="isHoveringUpload = false"
                  @dragenter.prevent="handleDragEnter"
                  @dragleave.prevent="handleDragLeave"
                  @dragover.prevent
                  @drop.prevent="handleDrop">
                  <img v-if="metadataDraft.thumbnail" :src="metadataDraft.thumbnail" class="w-full h-full object-cover" />
                  <div v-else class="w-full h-full flex items-center justify-center bg-theme-bg-item">
                    <svg-sprite src="DefaultCover" class="w-6 h-6 text-theme-text-disabled" />
                  </div>
                  <label
                    class="absolute inset-0 bg-black/70 flex items-center justify-center cursor-pointer text-[9px] text-cyan-accent-3 font-semibold select-none transition-all duration-200"
                    :class="[
                      isDraggingOver
                        ? 'opacity-100 bg-black/85 border-2 border-dashed border-theme-accent!'
                        : 'opacity-0 group-hover:opacity-100',
                    ]">
                    <span v-if="isDraggingOver">Drop Cover Here</span>
                    <span v-else class="text-center px-2">
                      Click to Upload
                      <br />
                      or Drag & Drop / Paste
                    </span>
                    <input type="file" accept="image/*" class="hidden" @change="handleThumbnailChange" />
                  </label>
                </div>
                <div class="flex-1 min-w-0 text-left">
                  <div class="text-xs text-white font-semibold mb-0.5">Cover Image</div>
                  <div class="text-[9px] text-theme-text-muted leading-tight">Pick a PNG, JPG, or WEBP to embed as cover art.</div>
                  <v-btn
                    v-if="metadataDraft.thumbnail"
                    @click="metadataDraft.thumbnail = ''"
                    variant="outlined"
                    density="compact"
                    class="text-[9px] border-theme-border! text-red-400! mt-1.5 h-auto py-0.5 px-1.5 text-none">
                    Remove Cover
                  </v-btn>
                </div>
              </div>

              <!-- Compact text inputs -->
              <div class="grid grid-cols-2 gap-3 w-stretch">
                <label class="col-span-2 text-left">
                  <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Filename (Name)</div>
                  <v-text-field
                    v-model="metadataDraft.filename"
                    placeholder="e.g. SANYO.mp3"
                    density="compact"
                    variant="outlined"
                    color="cyan-accent-3"
                    hide-details />
                </label>

                <label class="col-span-2 text-left">
                  <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Title</div>
                  <v-text-field
                    v-model="metadataDraft.title"
                    placeholder="Title"
                    density="compact"
                    variant="outlined"
                    color="cyan-accent-3"
                    hide-details />
                </label>

                <label class="text-left">
                  <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Album</div>
                  <v-text-field
                    v-model="metadataDraft.album_name"
                    placeholder="Album"
                    density="compact"
                    variant="outlined"
                    color="cyan-accent-3"
                    hide-details />
                </label>

                <!-- <label class="text-left">
									<div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Artist</div>
									<v-text-field v-model="metadataDraft.artist" placeholder="Artist" density="compact" variant="outlined" color="cyan-accent-3" hide-details />
								</label> -->

                <label class="text-left">
                  <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Track Number (#)</div>
                  <v-text-field
                    v-model="metadataDraft.track_number"
                    placeholder="e.g. 1"
                    density="compact"
                    variant="outlined"
                    color="cyan-accent-3"
                    hide-details />
                </label>
              </div>
            </div>
          </overlay-scrollbars-component>
        </v-card-text>
        <v-card-actions class="px-4 py-3 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
          <custom-btn variant="secondary" @click="isEditingMetadata = false" :disabled="isSavingMetadata">Cancel</custom-btn>
          <custom-btn variant="primary" @click="handleSaveMetadata" :loading="isSavingMetadata">Save</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>

    <!-- Timeline Edit Dialog -->
    <v-dialog v-model="isEditingTimeline" max-width="500px">
      <v-card class="bg-theme-bg-card border border-theme-border/60 text-theme-text rounded-2xl overflow-hidden p-4 shadow-xl">
        <v-card-title class="px-2 pb-2 text-base font-bold border-b border-theme-border/30">Edit Timeline</v-card-title>
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
            class="w-full bg-theme-bg-item/40 border border-theme-border focus:border-theme-accent/50 rounded-xl p-3 text-xs text-theme-text-secondary outline-hidden resize-none font-mono leading-relaxed transition-colors focus:ring-1 focus:ring-theme-accent/20"></textarea>
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
  import { ref, computed, watch, nextTick, onMounted, onBeforeUnmount } from 'vue'
  import { storeToRefs } from 'pinia'
  import { invoke } from '@tauri-apps/api/core'
  import { useAudioPlayer } from '@groovex/state'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { formatDuration } from '@groovex/core'
  import PlaylistHistory from './components/playlist-history.vue'

  const player = useAudioPlayer()
  const { isPlaying, currentSong, currentTime, duration } = storeToRefs(player)

  const activeTimelineRowRef = ref<HTMLElement | null>(null)
  const timelineOsInstance = ref<any>(null)
  const dynamicAccentColor = ref('var(--color-theme-accent-glow)')

  // Edit states (lyrics & timeline)
  const isEditingLyrics = ref(false)
  const lyricsDraft = ref('')
  const isSavingLyrics = ref(false)
  const lyricsEditorRef = ref<HTMLDivElement | null>(null)

  const isEditingTimeline = ref(false)
  const timelineDraft = ref('')
  const isSavingTimeline = ref(false)

  const isEditingMetadata = ref(false)
  const isSavingMetadata = ref(false)
  const metadataDraft = ref({
    filename: '',
    title: '',
    artist: '',
    album_name: '',
    track_number: '',
    thumbnail: '',
  })

  const isHoveringUpload = ref(false)
  const isDraggingOver = ref(false)

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
    nextTick(() => {
      if (lyricsEditorRef.value) {
        lyricsEditorRef.value.innerText = lyricsDraft.value
      }
    })
  }

  function handleLyricsInput(e: Event) {
    const target = e.target as HTMLDivElement
    lyricsDraft.value = target.innerText
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

  async function startEditMetadata() {
    if (!currentSong.value) return
    try {
      const meta = await invoke<any>('get_song_metadata', { songId: currentSong.value.id })
      metadataDraft.value = {
        filename: meta.filename,
        title: meta.title || '',
        artist: meta.artist || '',
        album_name: meta.album_name || '',
        track_number: meta.track_number || '',
        thumbnail: meta.thumbnail || '',
      }
      isEditingMetadata.value = true
    } catch (err) {
      console.error('Failed to get song metadata:', err)
    }
  }

  function readImageFile(file: File) {
    const reader = new FileReader()
    reader.onload = (event) => {
      metadataDraft.value.thumbnail = event.target?.result as string
    }
    reader.readAsDataURL(file)
  }

  function handleThumbnailChange(e: Event) {
    const target = e.target as HTMLInputElement
    const file = target.files?.[0]
    if (file) {
      readImageFile(file)
    }
  }

  let dragCounter = 0
  function handleDragEnter() {
    dragCounter++
    isDraggingOver.value = true
  }

  function handleDragLeave() {
    dragCounter--
    if (dragCounter === 0) {
      isDraggingOver.value = false
    }
  }

  function handleDrop(e: DragEvent) {
    dragCounter = 0
    isDraggingOver.value = false
    const file = e.dataTransfer?.files?.[0]
    if (file && file.type.startsWith('image/')) {
      readImageFile(file)
    }
  }

  function handlePaste(e: ClipboardEvent) {
    if (!isEditingMetadata.value || !isHoveringUpload.value) return

    const items = e.clipboardData?.items
    if (!items) return

    for (const item of items) {
      if (item.type.indexOf('image') !== -1) {
        const file = item.getAsFile()
        if (file) {
          readImageFile(file)
          e.preventDefault()
          break
        }
      }
    }
  }

  onMounted(() => {
    window.addEventListener('paste', handlePaste)
  })

  onBeforeUnmount(() => {
    window.removeEventListener('paste', handlePaste)
  })

  async function handleSaveMetadata() {
    if (!currentSong.value) return
    isSavingMetadata.value = true
    try {
      await player.updateSongMetadata(currentSong.value.id, {
        filename: metadataDraft.value.filename,
        title: metadataDraft.value.title,
        artist: metadataDraft.value.artist,
        album_name: metadataDraft.value.album_name,
        track_number: metadataDraft.value.track_number,
        thumbnail: metadataDraft.value.thumbnail,
      })
      isEditingMetadata.value = false
    } catch (err) {
      console.error('Failed to save song metadata:', err)
    } finally {
      isSavingMetadata.value = false
    }
  }

  // Reset editing mode when song changes
  watch(currentSong, () => {
    isEditingLyrics.value = false
    isEditingTimeline.value = false
    isEditingMetadata.value = false
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
  const thumbnailUrl = computed(() => currentSong.value?.thumbnail || null)

  function onTimelineOsInitialized(instance: any) {
    timelineOsInstance.value = instance
  }

  function setActiveTimelineRowRef(el: any, idx: number) {
    if (idx === activeSegmentIndex.value) {
      activeTimelineRowRef.value = el
    }
  }

  function scrollToActiveTimeline() {
    if (timelineOsInstance.value && activeTimelineRowRef.value) {
      const { viewport } = timelineOsInstance.value.elements()
      const activeEl = activeTimelineRowRef.value

      const viewportHeight = viewport.clientHeight
      const elementHeight = activeEl.clientHeight

      const targetTop =
        activeEl.getBoundingClientRect().top -
        viewport.getBoundingClientRect().top +
        viewport.scrollTop -
        viewportHeight / 2 +
        elementHeight / 2

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

  // Watch activeSegmentIndex to scroll the active segment into the center of viewport
  watch(
    [activeSegmentIndex, activeTimelineRowRef],
    ([idx, activeEl]) => {
      if (idx !== -1 && activeEl) {
        nextTick(() => {
          scrollToActiveTimeline()
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
  .timeline-scroll-container .os-viewport {
    scroll-snap-type: y mandatory;
    scroll-behavior: smooth;
  }
</style>
