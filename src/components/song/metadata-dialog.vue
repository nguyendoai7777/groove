<template>
  <v-dialog v-model="dialogVisible" max-width="800">
    <v-card class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
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
                  placeholder="e.g. SANYO"
                  density="compact"
                  variant="outlined"
                  color="cyan-accent-3"
                  :suffix="fileExtension"
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
        <custom-btn variant="secondary" @click="dialogVisible = false" :disabled="isSavingMetadata">Cancel</custom-btn>
        <custom-btn variant="primary" @click="handleSaveMetadata" :loading="isSavingMetadata">Save</custom-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue'
  import { invoke } from '@tauri-apps/api/core'
  import { useAudioPlayer } from '@groovex/store'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'

  const props = defineProps<{
    modelValue: boolean
    songId: number | undefined | null
  }>()

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void
    (e: 'saved', updatedSong: any): void
  }>()

  const player = useAudioPlayer()

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  })

  const isSavingMetadata = ref(false)
  const isHoveringUpload = ref(false)
  const isDraggingOver = ref(false)
  let dragCounter = 0

  const metadataDraft = ref({
    filename: '',
    title: '',
    artist: '',
    album_name: '',
    track_number: '',
    thumbnail: '',
  })

  const fileExtension = ref('')

  async function loadMetadata() {
    if (!props.songId) return
    try {
      const meta = await invoke<any>('get_song_metadata', { songId: props.songId })

      const filename = meta.filename || ''
      const lastDot = filename.lastIndexOf('.')
      if (lastDot !== -1) {
        metadataDraft.value.filename = filename.substring(0, lastDot)
        fileExtension.value = filename.substring(lastDot)
      } else {
        metadataDraft.value.filename = filename
        fileExtension.value = ''
      }

      metadataDraft.value.title = meta.title || ''
      metadataDraft.value.artist = meta.artist || ''
      metadataDraft.value.album_name = meta.album_name || ''
      metadataDraft.value.track_number = meta.track_number || ''
      metadataDraft.value.thumbnail = meta.thumbnail || ''
    } catch (err) {
      console.error('Failed to get song metadata:', err)
    }
  }

  function stripExtension(filename: string, originalExt: string): string {
    if (!filename) return ''

    // 1. Check original extension (case-insensitive)
    if (originalExt) {
      const extEscaped = originalExt.replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&')
      const regex = new RegExp(extEscaped + '$', 'i')
      if (regex.test(filename)) {
        return filename.replace(regex, '')
      }
    }

    // 2. Check other common audio extensions (case-insensitive)
    const AUDIO_EXTENSIONS = ['.mp3', '.wav', '.flac', '.m4a', '.aac', '.ogg', '.opus', '.wma', '.mp4', '.m4r', '.webm']
    for (const ext of AUDIO_EXTENSIONS) {
      const extEscaped = ext.replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&')
      const regex = new RegExp(extEscaped + '$', 'i')
      if (regex.test(filename)) {
        return filename.replace(regex, '')
      }
    }

    return filename
  }

  watch(
    () => metadataDraft.value.filename,
    (newVal) => {
      const stripped = stripExtension(newVal, fileExtension.value)
      if (stripped !== newVal) {
        metadataDraft.value.filename = stripped
      }
    },
  )

  watch(
    () => props.modelValue,
    async (isOpen) => {
      if (isOpen && props.songId) {
        await loadMetadata()
      }
    },
  )

  watch(
    () => props.songId,
    async (newId) => {
      if (props.modelValue && newId) {
        await loadMetadata()
      }
    },
  )

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
    if (!dialogVisible.value || !isHoveringUpload.value) return

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
    if (!props.songId) return
    isSavingMetadata.value = true
    try {
      const fullFilename = metadataDraft.value.filename + fileExtension.value
      const updatedSong = await player.updateSongMetadata(props.songId, {
        filename: fullFilename,
        title: metadataDraft.value.title,
        artist: metadataDraft.value.artist,
        album_name: metadataDraft.value.album_name,
        track_number: metadataDraft.value.track_number,
        thumbnail: metadataDraft.value.thumbnail,
      })
      emit('saved', updatedSong)
      dialogVisible.value = false
    } catch (err) {
      console.error('Failed to save song metadata:', err)
    } finally {
      isSavingMetadata.value = false
    }
  }
</script>

<style>
  .grx-ConfirmerCard {
    background-color: var(--color-theme-bg-item) !important;
    color: var(--color-theme-text) !important;
    border: 1px solid var(--color-theme-border) !important;
    border-radius: 12px !important;
  }
</style>
