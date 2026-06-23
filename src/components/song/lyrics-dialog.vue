<template>
  <v-dialog v-model="dialogVisible" max-width="500px">
    <v-card class="bg-theme-bg-card border border-theme-border/60 text-theme-text rounded-2xl overflow-hidden p-4 shadow-xl">
      <v-card-title class="px-2 pb-2 text-base font-bold border-b border-theme-border/30">Edit Lyrics</v-card-title>
      <v-card-text class="px-2 py-4">
        <textarea
          v-model="lyricsDraft"
          placeholder="Paste or write lyrics here..."
          rows="12"
          class="w-full bg-theme-bg-item/40 border border-theme-border focus:border-theme-accent/50 rounded-xl p-3 text-xs text-theme-text-secondary outline-hidden resize-none font-sans leading-relaxed transition-colors focus:ring-1 focus:ring-theme-accent/20"></textarea>
      </v-card-text>
      <v-card-actions class="px-2 pt-2 border-t border-theme-border/30 justify-end gap-2">
        <custom-btn variant="secondary" @click="dialogVisible = false" :disabled="isSavingLyrics">Cancel</custom-btn>
        <custom-btn variant="primary" @click="handleSaveLyrics" :loading="isSavingLyrics">Save</custom-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { invoke } from '@tauri-apps/api/core'
  import { useAudioPlayer } from '@groovex/state'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'

  const props = defineProps<{
    modelValue: boolean
    songId: number | undefined | null
  }>()

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void
    (e: 'saved', lyrics: string): void
  }>()

  const player = useAudioPlayer()

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  })

  const isSavingLyrics = ref(false)
  const lyricsDraft = ref('')

  async function loadLyrics() {
    if (!props.songId) return
    try {
      const meta = await invoke<any>('get_song_metadata', { songId: props.songId })
      lyricsDraft.value = meta.lyrics || ''
    } catch (err) {
      console.error('Failed to get song lyrics:', err)
    }
  }

  watch(
    () => props.modelValue,
    async (isOpen) => {
      if (isOpen) {
        await loadLyrics()
      }
    },
  )

  watch(
    () => props.songId,
    async (newId) => {
      if (props.modelValue && newId) {
        await loadLyrics()
      }
    },
  )

  async function handleSaveLyrics() {
    if (!props.songId) return
    isSavingLyrics.value = true
    try {
      await invoke('update_song_lyrics', {
        songId: props.songId,
        lyrics: lyricsDraft.value,
      })
      player.updateLyrics(props.songId, lyricsDraft.value)
      emit('saved', lyricsDraft.value)
      dialogVisible.value = false
    } catch (err) {
      console.error('Failed to save lyrics:', err)
    } finally {
      isSavingLyrics.value = false
    }
  }
</script>
