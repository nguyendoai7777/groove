<template>
  <v-dialog v-model="dialogVisible" max-width="900">
    <v-card
      class="grx-MetadataDialog grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
      <v-card-title
        class="px-4 py-3 text-base font-bold border-b border-theme-border/30 bg-theme-bg-placeholder/20 flex items-center gap-3">
        <span>Edit Metadata</span>
        <span v-if="junkFrames.length" class="text-[10px] font-medium text-amber-400">
          {{ junkFrames.length }} junk frame{{ junkFrames.length > 1 ? 's' : '' }} detected
        </span>
      </v-card-title>

      <v-card-text class="px-1">
        <overlay-scrollbars-component :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="p-3 max-h-[70svh]">
          <div class="flex gap-4 items-start">
            <!-- Thumbnail cover selection -->
            <div class="flex flex-col gap-4 items-center shrink-0">
              <div
                class="relative w-[200px] h-[200px] rounded-lg overflow-hidden bg-theme-bg-placeholder border border-theme-border/50 shrink-0 group"
                @mouseenter="isHoveringUpload = true"
                @mouseleave="isHoveringUpload = false"
                @dragenter.prevent="handleDragEnter"
                @dragleave.prevent="handleDragLeave"
                @dragover.prevent
                @drop.prevent="handleDrop">
                <img v-if="draft.thumbnail" :src="draft.thumbnail" class="w-full h-full object-cover" />
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
                    or Drag &amp; Drop / Paste
                  </span>
                  <input type="file" accept="image/*" class="hidden" @change="handleThumbnailChange" />
                </label>
              </div>
              <div class="w-[200px] text-left">
                <div class="text-xs text-white font-semibold mb-0.5">Cover Image</div>
                <div class="text-[9px] text-theme-text-muted leading-tight">Pick a PNG, JPG, or WEBP to embed as cover art.</div>
                <v-btn
                  v-if="draft.thumbnail"
                  @click="draft.thumbnail = ''"
                  variant="outlined"
                  density="compact"
                  class="text-[9px] border-theme-border! text-red-400! mt-1.5 h-auto py-0.5 px-1.5 text-none">
                  Remove Cover
                </v-btn>
              </div>
            </div>

            <div class="flex-1 min-w-0">
              <v-tabs v-model="activeTab" density="compact" color="cyan-accent-3" class="mb-3 border-b border-theme-border/30">
                <v-tab value="common" class="text-xs text-none">Common</v-tab>
                <v-tab value="details" class="text-xs text-none">Details</v-tab>
                <v-tab value="raw" class="text-xs text-none">
                  Raw Frames
                  <span v-if="junkFrames.length" class="ml-1 text-amber-400">({{ junkFrames.length }})</span>
                </v-tab>
              </v-tabs>

              <!-- COMMON -->
              <div v-show="activeTab === 'common'" class="grid grid-cols-2 gap-3">
                <label class="col-span-2 text-left">
                  <div :class="labelClass">Filename (Name)</div>
                  <v-text-field
                    v-model="filenameDraft"
                    placeholder="e.g. SANYO"
                    density="compact"
                    variant="outlined"
                    color="cyan-accent-3"
                    :suffix="fileExtension"
                    hide-details />
                </label>

                <label class="col-span-2 text-left">
                  <div :class="labelClass">Title</div>
                  <v-text-field v-model="draft.title" placeholder="Title" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Artist</div>
                  <v-text-field v-model="draft.artist" placeholder="Track artist" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Album</div>
                  <v-text-field v-model="draft.album_name" placeholder="Album" v-bind="fieldProps" />
                </label>

                <label class="col-span-2 text-left">
                  <div :class="labelClass">
                    Album Artist
                    <span class="text-theme-text-muted font-normal normal-case">— this is what groups albums</span>
                  </div>
                  <v-text-field
                    v-model="draft.album_artist"
                    placeholder="Leave identical across every track of the album"
                    v-bind="fieldProps" />
                </label>

                <!--
                  iTunes, Plex and the Windows shell group by (Album, Album Artist) and
                  fall back to the per-track artist when Album Artist is empty, which is
                  what splits one album into one entry per featured artist.
                -->
                <div
                  v-if="groupingWarning"
                  class="col-span-2 flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-left">
                  <div class="flex-1 text-[10px] leading-relaxed text-amber-200">
                    {{ groupingWarning }}
                  </div>
                  <v-btn
                    variant="outlined"
                    density="compact"
                    class="text-[9px] border-amber-400/60! text-amber-200! h-auto py-1 px-2 text-none shrink-0"
                    @click="fixGrouping">
                    Fix grouping
                  </v-btn>
                </div>

                <label class="text-left">
                  <div :class="labelClass">Track # / Total</div>
                  <div class="flex items-center gap-2">
                    <v-text-field v-model="draft.track_number" placeholder="1" v-bind="fieldProps" />
                    <span class="text-theme-text-muted text-xs">/</span>
                    <v-text-field v-model="draft.track_total" placeholder="12" v-bind="fieldProps" />
                  </div>
                </label>

                <label class="text-left">
                  <div :class="labelClass">Disc # / Total</div>
                  <div class="flex items-center gap-2">
                    <v-text-field v-model="draft.disc_number" placeholder="1" v-bind="fieldProps" />
                    <span class="text-theme-text-muted text-xs">/</span>
                    <v-text-field v-model="draft.disc_total" placeholder="1" v-bind="fieldProps" />
                  </div>
                </label>
              </div>

              <!-- DETAILS -->
              <div v-show="activeTab === 'details'" class="grid grid-cols-2 gap-3">
                <label class="text-left">
                  <div :class="labelClass">Year</div>
                  <v-text-field v-model="draft.year" placeholder="2016" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Genre</div>
                  <v-text-field v-model="draft.genre" placeholder="Pop" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Composer</div>
                  <v-text-field v-model="draft.composer" placeholder="Composer" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Publisher</div>
                  <v-text-field v-model="draft.publisher" placeholder="Label" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">Copyright</div>
                  <v-text-field v-model="draft.copyright" placeholder="Leave empty to erase" v-bind="fieldProps" />
                </label>

                <label class="text-left">
                  <div :class="labelClass">BPM</div>
                  <v-text-field v-model="draft.bpm" placeholder="120" v-bind="fieldProps" />
                </label>

                <label class="col-span-2 text-left">
                  <div :class="labelClass">Grouping</div>
                  <v-text-field v-model="draft.grouping" placeholder="Grouping" v-bind="fieldProps" />
                </label>

                <label class="col-span-2 text-left">
                  <div :class="labelClass">Comment</div>
                  <v-textarea v-model="draft.comment" placeholder="Comment" rows="2" v-bind="fieldProps" />
                </label>

                <label class="col-span-2 flex items-center gap-2 text-left cursor-pointer">
                  <input type="checkbox" v-model="draft.compilation" class="accent-cyan-400 w-3.5 h-3.5" />
                  <span class="text-xs text-theme-text-secondary">
                    Part of a compilation
                    <span class="text-theme-text-muted">— keeps multi-artist albums together in iTunes</span>
                  </span>
                </label>
              </div>

              <!-- RAW FRAMES -->
              <div v-show="activeTab === 'raw'" class="text-left">
                <div class="text-[10px] text-theme-text-muted mb-2 leading-relaxed">
                  Everything currently stored in the file. Frames marked
                  <span class="text-amber-400 font-semibold">junk</span>
                  are what make players disagree about grouping; a Clean or Clean + keep known save removes them.
                </div>
                <div v-if="!rawFrames.length" class="text-xs text-theme-text-muted py-4 text-center">No frames found.</div>
                <table v-else class="w-full text-[10px]">
                  <tbody>
                    <tr v-for="(frame, i) in rawFrames" :key="`${frame.id}-${i}`" class="border-b border-theme-border/20">
                      <td
                        class="py-1.5 pr-2 font-mono align-top whitespace-nowrap"
                        :class="frame.known ? 'text-cyan-300' : 'text-amber-400'">
                        {{ frame.id }}
                      </td>
                      <td class="py-1.5 pr-2 align-top whitespace-nowrap text-theme-text-muted">{{ frame.name }}</td>
                      <td class="py-1.5 align-top break-all text-theme-text-secondary">{{ frame.value }}</td>
                    </tr>
                  </tbody>
                </table>
              </div>
            </div>
          </div>
        </overlay-scrollbars-component>
      </v-card-text>

      <div class="px-4 py-2.5 border-t border-theme-border/30 bg-theme-bg-placeholder/10 flex flex-col gap-2 text-left">
        <div class="flex items-center gap-3 flex-wrap">
          <span class="text-[10px] text-theme-text-secondary font-medium shrink-0">Write mode</span>
          <v-select
            v-model="writeMode"
            :items="writeModes"
            item-title="label"
            item-value="value"
            density="compact"
            variant="outlined"
            color="cyan-accent-3"
            hide-details
            class="max-w-[280px] text-xs" />
          <span class="text-[10px] text-theme-text-muted flex-1 min-w-[200px]">{{ activeModeHint }}</span>
        </div>

        <label v-if="canBulkApply" class="flex items-center gap-2 cursor-pointer">
          <input type="checkbox" v-model="applyToCategory" class="accent-cyan-400 w-3.5 h-3.5" />
          <span class="text-[10px] text-theme-text-secondary">
            Also apply album-level fields to all
            <b>{{ categorySongIds.length }}</b>
            tracks in this {{ categoryType }}
            <span class="text-theme-text-muted">(Album, Album Artist, Genre, Year, Compilation, Cover)</span>
          </span>
        </label>

        <div v-if="saveError" class="text-[10px] text-red-400">{{ saveError }}</div>
        <div v-if="saveNotice" class="text-[10px] text-emerald-400">{{ saveNotice }}</div>
      </div>

      <v-card-actions class="px-4 py-3 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
        <custom-btn variant="secondary" @click="dialogVisible = false" :disabled="isSaving">Cancel</custom-btn>
        <custom-btn variant="primary" @click="handleSave" :loading="isSaving">Save</custom-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch, onMounted, onBeforeUnmount } from 'vue';
  import { invoke } from '@tauri-apps/api/core';
  import { useAudioPlayer } from '@groovex/store';
  import type { FullMetadata, MetadataWriteMode, RawFrame, SongMetadata } from '@groovex/types';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import CustomBtn from '@groovex/ui/button/custom-btn.vue';
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue';

  const props = defineProps<{
    modelValue: boolean;
    songId: number | undefined | null;
    /** Category the song is being edited from, enabling "apply to every track". */
    categoryType?: 'album' | 'folder';
    categoryId?: number;
  }>();

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void;
    (e: 'saved', updatedSong: any): void;
    (e: 'bulk-saved', result: { updated: number; failed: string[] }): void;
  }>();

  const player = useAudioPlayer();

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  });

  const writeModes: { value: MetadataWriteMode; label: string; hint: string }[] = [
    {
      value: 'clean_keep',
      label: 'Clean + keep known fields',
      hint: 'Rewrites the tag as ID3v2.3, drops junk frames and the ID3v1/APE leftovers, keeps every field shown here.',
    },
    {
      value: 'clean',
      label: 'Clean (only fields below)',
      hint: 'Writes nothing but what is in this form. Anything not shown here is erased from the file.',
    },
    {
      value: 'merge',
      label: 'Merge (leave other frames)',
      hint: 'Only touches the fields you changed. Junk frames and legacy tags stay in the file.',
    },
  ];

  const labelClass = 'block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide';
  const fieldProps = {
    density: 'compact' as const,
    variant: 'outlined' as const,
    color: 'cyan-accent-3',
    hideDetails: true,
  };

  const activeTab = ref<'common' | 'details' | 'raw'>('common');
  const writeMode = ref<MetadataWriteMode>('clean_keep');
  const isSaving = ref(false);
  const isHoveringUpload = ref(false);
  const isDraggingOver = ref(false);
  const saveError = ref('');
  const saveNotice = ref('');
  let dragCounter = 0;

  const rawFrames = ref<RawFrame[]>([]);
  const categorySongIds = ref<number[]>([]);
  const applyToCategory = ref(false);

  function emptyDraft(): SongMetadata {
    return {
      title: '',
      artist: '',
      album_name: '',
      album_artist: '',
      track_number: '',
      track_total: '',
      disc_number: '',
      disc_total: '',
      year: '',
      genre: '',
      composer: '',
      publisher: '',
      copyright: '',
      comment: '',
      bpm: '',
      grouping: '',
      compilation: false,
      thumbnail: '',
    };
  }

  const draft = ref<SongMetadata>(emptyDraft());
  const filenameDraft = ref('');
  const fileExtension = ref('');

  const junkFrames = computed(() => rawFrames.value.filter((f) => !f.known));
  const activeModeHint = computed(() => writeModes.find((m) => m.value === writeMode.value)?.hint ?? '');
  const canBulkApply = computed(() => !!props.categoryType && categorySongIds.value.length > 1);

  const groupingWarning = computed(() => {
    const album = draft.value.album_name?.trim();
    if (!album) return '';
    if (!draft.value.album_artist?.trim()) {
      return `Album Artist is empty, so iTunes and Plex fall back to the track artist and split "${album}" into one album per artist.`;
    }
    return '';
  });

  /** Gives every track of the album the same grouping key. */
  function fixGrouping() {
    const album = draft.value.album_name?.trim();
    if (!album) return;
    draft.value.album_artist = album;
    draft.value.compilation = true;
    if (canBulkApply.value) applyToCategory.value = true;
  }

  async function loadMetadata() {
    if (!props.songId) return;
    saveError.value = '';
    saveNotice.value = '';
    try {
      const meta = await invoke<FullMetadata>('get_song_metadata', { songId: props.songId });

      const lastDot = (meta.filename || '').lastIndexOf('.');
      if (lastDot !== -1) {
        filenameDraft.value = meta.filename.substring(0, lastDot);
        fileExtension.value = meta.filename.substring(lastDot);
      } else {
        filenameDraft.value = meta.filename || '';
        fileExtension.value = '';
      }

      const next = emptyDraft();
      for (const key of Object.keys(next) as (keyof SongMetadata)[]) {
        const value = meta[key];
        if (key === 'compilation') {
          next.compilation = value === true;
        } else if (typeof value === 'string') {
          (next[key] as string) = value;
        }
      }
      draft.value = next;
      rawFrames.value = meta.raw_frames ?? [];
    } catch (err) {
      console.error('Failed to get song metadata:', err);
      saveError.value = String(err);
    }
  }

  async function loadCategorySongs() {
    categorySongIds.value = [];
    applyToCategory.value = false;
    if (!props.categoryType || props.categoryId == null) return;
    try {
      categorySongIds.value = await player.getCategorySongIds(props.categoryType, props.categoryId);
    } catch (err) {
      console.error('Failed to list category songs:', err);
    }
  }

  function stripExtension(filename: string, originalExt: string): string {
    if (!filename) return '';

    // 1. Check original extension (case-insensitive)
    if (originalExt) {
      const extEscaped = originalExt.replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&');
      const regex = new RegExp(extEscaped + '$', 'i');
      if (regex.test(filename)) {
        return filename.replace(regex, '');
      }
    }

    // 2. Check other common audio extensions (case-insensitive)
    const AUDIO_EXTENSIONS = ['.mp3', '.wav', '.flac', '.m4a', '.aac', '.ogg', '.opus', '.wma', '.mp4', '.m4r', '.webm'];
    for (const ext of AUDIO_EXTENSIONS) {
      const extEscaped = ext.replace(/[-\/\\^$*+?.()|[\]{}]/g, '\\$&');
      const regex = new RegExp(extEscaped + '$', 'i');
      if (regex.test(filename)) {
        return filename.replace(regex, '');
      }
    }

    return filename;
  }

  watch(filenameDraft, (newVal) => {
    const stripped = stripExtension(newVal, fileExtension.value);
    if (stripped !== newVal) {
      filenameDraft.value = stripped;
    }
  });

  watch(
    () => props.modelValue,
    async (isOpen) => {
      if (isOpen && props.songId) {
        activeTab.value = 'common';
        await Promise.all([loadMetadata(), loadCategorySongs()]);
      }
    },
  );

  watch(
    () => props.songId,
    async (newId) => {
      if (props.modelValue && newId) {
        await Promise.all([loadMetadata(), loadCategorySongs()]);
      }
    },
  );

  function readImageFile(file: File) {
    const reader = new FileReader();
    reader.onload = (event) => {
      draft.value.thumbnail = event.target?.result as string;
    };
    reader.readAsDataURL(file);
  }

  function handleThumbnailChange(e: Event) {
    const target = e.target as HTMLInputElement;
    const file = target.files?.[0];
    if (file) {
      readImageFile(file);
    }
  }

  function handleDragEnter() {
    dragCounter++;
    isDraggingOver.value = true;
  }

  function handleDragLeave() {
    dragCounter--;
    if (dragCounter === 0) {
      isDraggingOver.value = false;
    }
  }

  function handleDrop(e: DragEvent) {
    dragCounter = 0;
    isDraggingOver.value = false;
    const file = e.dataTransfer?.files?.[0];
    if (file && file.type.startsWith('image/')) {
      readImageFile(file);
    }
  }

  function handlePaste(e: ClipboardEvent) {
    if (!dialogVisible.value || !isHoveringUpload.value) return;

    const items = e.clipboardData?.items;
    if (!items) return;

    for (const item of items) {
      if (item.type.indexOf('image') !== -1) {
        const file = item.getAsFile();
        if (file) {
          readImageFile(file);
          e.preventDefault();
          break;
        }
      }
    }
  }

  onMounted(() => {
    window.addEventListener('paste', handlePaste);
  });

  onBeforeUnmount(() => {
    window.removeEventListener('paste', handlePaste);
  });

  /**
   * Sends every field, empty ones included: an empty string is the backend's
   * "erase this frame", which is how a stubborn value such as copyright is removed
   * instead of being carried forward from the file.
   */
  function buildPayload(): SongMetadata {
    return { ...draft.value };
  }

  function buildAlbumPayload(): SongMetadata {
    return {
      album_name: draft.value.album_name,
      album_artist: draft.value.album_artist,
      genre: draft.value.genre,
      year: draft.value.year,
      compilation: draft.value.compilation,
      thumbnail: draft.value.thumbnail,
    };
  }

  async function handleSave() {
    if (!props.songId) return;
    isSaving.value = true;
    saveError.value = '';
    saveNotice.value = '';
    try {
      const updatedSong = await player.updateSongMetadata(
        props.songId,
        filenameDraft.value + fileExtension.value,
        buildPayload(),
        writeMode.value,
      );

      if (applyToCategory.value && categorySongIds.value.length) {
        // The edited track already carries the final values; re-writing it here
        // would only undo the per-track fields the bulk payload omits.
        const others = categorySongIds.value.filter((id) => id !== props.songId);
        // Never bulk-write in 'clean' mode: the album payload has no title or track
        // number, so a clean write would erase those on every other track.
        const bulkMode: MetadataWriteMode = writeMode.value === 'merge' ? 'merge' : 'clean_keep';
        const result = await player.applyMetadataToSongs(others, buildAlbumPayload(), bulkMode);
        emit('bulk-saved', result);
        if (result.failed.length) {
          saveError.value = `${result.failed.length} file(s) failed: ${result.failed[0]}`;
          isSaving.value = false;
          return;
        }
        saveNotice.value = `Updated ${result.updated + 1} tracks.`;
      }

      emit('saved', updatedSong);
      dialogVisible.value = false;
    } catch (err) {
      console.error('Failed to save song metadata:', err);
      saveError.value = String(err);
    } finally {
      isSaving.value = false;
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
