<template>
  <v-dialog v-model="dialogVisible" max-width="500px">
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
        <overlay-scrollbars-component
          :options="{ scrollbars: { autoHide: 'scroll' } }"
          defer
          class="w-full h-48 bg-theme-bg-item/40 border border-theme-border focus-within:border-theme-accent/50 rounded-xl transition-colors focus-within:ring-1 focus-within:ring-theme-accent/20">
          <div
            ref="timelineEditorRef"
            contenteditable="true"
            placeholder="e.g. 0:00 First Track"
            class="w-full min-h-full p-3 text-xs text-theme-text-secondary outline-hidden font-mono leading-relaxed whitespace-pre-wrap wrap-break-word empty:before:content-[attr(placeholder)] empty:before:text-theme-text-disabled/60 empty:before:pointer-events-none"
            @input="handleTimelineInput"></div>
        </overlay-scrollbars-component>
      </v-card-text>
      <v-card-actions class="px-2 pt-2 border-t border-theme-border/30 justify-end gap-2">
        <custom-btn variant="secondary" @click="dialogVisible = false" :disabled="isSavingTimeline">Cancel</custom-btn>
        <custom-btn variant="primary" @click="handleSaveTimeline" :loading="isSavingTimeline">Save</custom-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick } from 'vue';
  import { invoke } from '@tauri-apps/api/core';
  import { useAudioPlayer } from '@groovex/store';
  import CustomBtn from '@groovex/ui/button/custom-btn.vue';
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue';

  const props = defineProps<{
    modelValue: boolean;
    songId: number | undefined | null;
    initialTimeline?: string | null;
  }>();

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void;
    (e: 'saved', timeline: string): void;
  }>();

  const player = useAudioPlayer();

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  });

  const isSavingTimeline = ref(false);
  const timelineDraft = ref('');
  const timelineEditorRef = ref<HTMLDivElement | null>(null);

  function handleTimelineInput(e: Event) {
    const target = e.target as HTMLDivElement;
    timelineDraft.value = target.innerText;
  }

  function updateEditorContent() {
    nextTick(() => {
      if (timelineEditorRef.value) {
        timelineEditorRef.value.innerText = timelineDraft.value;
      }
    });
  }

  async function loadTimeline() {
    if (!props.songId) return;
    if (props.initialTimeline !== undefined && props.initialTimeline !== null) {
      timelineDraft.value = props.initialTimeline;
      updateEditorContent();
      return;
    }
    try {
      const meta = await invoke<any>('get_song_metadata', { songId: props.songId });
      timelineDraft.value = meta.timeline || '';
      updateEditorContent();
    } catch (err) {
      console.error('Failed to get song timeline:', err);
    }
  }

  watch(
    () => props.modelValue,
    async (isOpen) => {
      if (isOpen) {
        await loadTimeline();
      }
    },
  );

  watch(
    () => props.songId,
    async (newId) => {
      if (props.modelValue && newId) {
        await loadTimeline();
      }
    },
  );

  async function handleSaveTimeline() {
    if (!props.songId) return;
    isSavingTimeline.value = true;
    try {
      await invoke('update_song_timeline', {
        songId: props.songId,
        timeline: timelineDraft.value,
      });
      player.updateTimeline(props.songId, timelineDraft.value);
      emit('saved', timelineDraft.value);
      dialogVisible.value = false;
    } catch (err) {
      console.error('Failed to save timeline:', err);
    } finally {
      isSavingTimeline.value = false;
    }
  }
</script>
