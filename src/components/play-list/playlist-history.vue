<template>
  <div
    class="w-now-playing-right-w min-w-now-playing-right-min-w max-w-now-playing-right-max-w flex flex-col h-full backdrop-blur-xs rounded-2xl border border-theme-border/50 overflow-hidden">
    <div class="flex items-center justify-between pb-2 border-b border-theme-border/30 shrink-0">
      <h3 class="text-sm font-bold text-white tracking-wide flex items-center gap-2 pt-3 px-3">
        <span>Queue / Playlist</span>
        <span class="text-[10px] font-normal text-theme-text-disabled">({{ playlistCount }} tracks)</span>
      </h3>
    </div>

    <!-- Queue List Viewport -->
    <div class="relative flex-1 min-h-0">
      <!-- Fade-out gradient at the top of scrollable area -->
      <!-- <div class="absolute top-0 left-0 right-0 h-3 bg-linear-to-b from-[#121212] to-transparent pointer-events-none z-10"></div> -->
      <overlay-scrollbars-component
        :options="{ scrollbars: { autoHide: 'scroll' } }"
        defer
        class="h-full px-1"
        @os-initialized="onQueueListOsInitialized">
        <div class="flex flex-col gap-1.5 px-2 pt-3">
          <template v-if="displayQueue.length > 0">
            <div
              v-for="(item, idx) in displayQueue"
              :key="item.song.id + '-' + idx"
              :id="'queue-item-' + item.song.id"
              :ref="(el) => setActiveRowRef(el, item)"
              class="group/item flex items-center gap-2.5 p-2 rounded-xl transition-all duration-200 cursor-pointer border border-transparent"
              :class="[
                item.isPlayed ? 'opacity-25 hover:opacity-50' : 'opacity-100',
                item.isCurrent
                  ? 'bg-theme-accent/10 border-theme-accent/30 shadow-md text-white'
                  : 'hover:bg-theme-bg-placeholder/40 text-theme-text-secondary',
              ]"
              @click="handlePlaySong(item.song)">
              <!-- Index / Status -->
              <div class="w-6 flex items-center justify-center text-[10px] font-mono text-theme-text-disabled">
                <span v-if="!item.isCurrent" class="group-hover/item:hidden">{{ idx + 1 }}</span>
                <svg-sprite
                  v-if="!item.isCurrent"
                  src="Play"
                  class="w-2.5 h-2.5 text-theme-accent-light fill-theme-accent-light hidden group-hover/item:block" />
                <playing-visualizer v-if="item.isCurrent" :paused="!isPlaying" class="w-3 h-3 text-theme-accent-light" />
              </div>

              <!-- Thumbnail -->
              <div class="w-8 h-8 rounded-md overflow-hidden bg-theme-bg-placeholder shrink-0 shadow-xs">
                <img v-if="item.song.thumbnail" :src="item.song.thumbnail" class="w-full h-full object-cover" />
                <div v-else class="w-full h-full flex items-center justify-center bg-theme-bg-item">
                  <svg-sprite src="Album" class="w-4 h-4 text-theme-text-disabled" />
                </div>
              </div>

              <!-- Title & Artist -->
              <div class="flex-1 min-w-0 text-left">
                <div
                  class="text-xs font-semibold truncate"
                  :class="item.isCurrent ? 'text-theme-accent-light' : 'text-theme-text-secondary group-hover/item:text-white'">
                  {{ item.song.title || item.song.filename.replace(/\.[^/.]+$/, '') }}
                </div>
                <div class="text-[10px] text-theme-text-muted truncate mt-0.5">
                  {{ item.song.artist || 'Unknown Artist' }}
                </div>
              </div>

              <!-- Duration -->
              <div class="text-[10px] font-mono text-theme-text-disabled pr-1 tabular-nums">
                {{ formatDuration(item.song.duration) }}
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
  import { ref, computed, watch, nextTick, type ComponentPublicInstance } from 'vue';
  import { storeToRefs } from 'pinia';
  import { useAudioPlayer, type PlaybackSong } from '@groovex/store';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue';
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue';
  import { formatDuration } from '@groovex/core';
  import { OverlayScrollbars } from 'overlayscrollbars';

  const player = useAudioPlayer();
  const { historyPlaylist, queuePlaylist, currentSong, isPlaying } = storeToRefs(player);

  const activeRowRef = ref<HTMLElement | null>(null);
  const queueListOsInstance = ref<OverlayScrollbars | null>(null);
  let skipNextScroll = false;

  interface DisplayItem {
    song: PlaybackSong;
    isPlayed: boolean;
    isCurrent: boolean;
  }

  const displayQueue = computed<DisplayItem[]>(() => {
    const list: DisplayItem[] = [];

    historyPlaylist.value.forEach((song) => {
      list.push({ song, isPlayed: true, isCurrent: false });
    });

    if (currentSong.value) {
      list.push({ song: currentSong.value, isPlayed: false, isCurrent: true });
    }

    queuePlaylist.value.forEach((song) => {
      list.push({ song, isPlayed: false, isCurrent: false });
    });

    return list;
  });

  const playlistCount = computed(() => {
    return displayQueue.value.length;
  });

  function handlePlaySong(song: PlaybackSong) {
    skipNextScroll = true;
    player.playSong(song);
    setTimeout(() => {
      skipNextScroll = false;
    }, 150);
  }

  function onQueueListOsInitialized(instance: OverlayScrollbars) {
    queueListOsInstance.value = instance;
  }

  function setActiveRowRef(el: Element | ComponentPublicInstance | null, item: DisplayItem) {
    console.log(`@@ elêmnt`, el);

    if (item.isCurrent) {
      activeRowRef.value = el as HTMLElement | null;
    }
  }

  function scrollToActive() {
    if (queueListOsInstance.value && activeRowRef.value) {
      const { viewport } = queueListOsInstance.value.elements();
      const activeEl = activeRowRef.value;

      const viewportHeight = viewport.clientHeight;
      const elementHeight = activeEl.clientHeight;

      const targetTop =
        activeEl.getBoundingClientRect().top -
        viewport.getBoundingClientRect().top +
        viewport.scrollTop -
        viewportHeight / 2 +
        elementHeight / 2;

      viewport.scrollTo({
        top: targetTop,
        behavior: 'smooth',
        // Use logical scroll positioning
      });
    }
  }

  // Watch state changes to dynamically scroll active row
  watch(
    [() => currentSong.value?.id, activeRowRef, queueListOsInstance],
    ([songId, activeEl, osInst]) => {
      if (songId && activeEl && osInst) {
        if (skipNextScroll) {
          return;
        }
        nextTick(() => {
          scrollToActive();
        });
      }
    },
    { immediate: true },
  );
</script>
