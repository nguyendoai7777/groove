<template>
  <div
    class="flex items-center px-3 py-2.5 rounded-lg cursor-pointer gap-3 transition-all duration-150 border-l-2"
    :class="
      isActive
        ? 'bg-(--cmd-item-bg-active) border-l-(--cmd-item-border-active) text-(--cmd-text-primary)'
        : 'border-l-transparent text-(--cmd-text-secondary) hover:bg-(--cmd-item-bg-active)/40'
    "
    @click="$emit('select', item)"
    @mouseenter="$emit('hover', item.globalIndex)">
    <span
      class="flex items-center justify-center w-7 h-7 rounded-md transition-colors duration-150"
      :class="
        isActive
          ? 'bg-(--cmd-item-icon-bg-active) text-(--cmd-item-icon-color-active)'
          : 'bg-(--cmd-item-icon-bg) text-(--cmd-item-icon-color)'
      ">
      <playing-visualizer
        v-if="item.type === 'song' && player.currentSong?.id === item.rawSong?.id"
        :paused="!player.isPlaying"
        class="w-3.5 h-3.5" />
      <svg-sprite v-else :src="getIconSrc(item.type)" class-name="w-3.5 h-3.5" />
    </span>

    <div class="flex flex-col flex-1 min-w-0">
      <span class="text-[13px] font-medium truncate" :class="isActive ? 'text-(--cmd-text-primary)' : 'text-(--cmd-text-primary)/90'">
        {{ item.title }}
      </span>
      <span
        class="text-xs truncate mt-0.5 transition-colors duration-150"
        :class="isActive ? 'text-(--cmd-text-primary)/75' : 'text-(--cmd-text-secondary)'">
        {{ item.description }}
      </span>
    </div>

    <span class="flex items-center">
      <kbd
        v-if="item.shortcut"
        class="font-mono text-[10px] font-medium rounded px-1.5 py-0.5 transition-colors duration-150 border"
        :class="
          isActive
            ? 'bg-(--cmd-kbd-bg-active) text-(--cmd-kbd-text-active) border-(--cmd-kbd-border-active)'
            : 'bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border-(--cmd-kbd-border) shadow-kbd-bottom'
        ">
        {{ item.shortcut }}
      </kbd>
      <kbd
        v-else-if="item.type === 'command'"
        class="font-mono text-[10px] font-medium rounded px-1.5 py-0.5 transition-colors duration-150 border"
        :class="
          isActive
            ? 'bg-(--cmd-kbd-bg-active) text-(--cmd-kbd-text-active) border-(--cmd-kbd-border-active)'
            : 'bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border-(--cmd-kbd-border) shadow-kbd-bottom'
        ">
        Tab Chọn
      </kbd>
    </span>
  </div>
</template>

<script setup lang="ts">
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import type { SVGSrc } from '@groovex/ui/svg-sprite'
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import { useAudioPlayer } from '@groovex/store'

  interface Props {
    item: {
      id: string
      type: string
      title: string
      description: string
      shortcut?: string
      globalIndex: number
      rawSong?: any
    }
    isActive: boolean
  }

  defineProps<Props>()
  defineEmits<{
    (e: 'select', item: Props['item']): void
    (e: 'hover', index: number): void
  }>()

  const player = useAudioPlayer()

  function getIconSrc(type: string): SVGSrc {
    switch (type) {
      case 'command':
        return 'Command'
      case 'song':
        return 'Song'
      case 'artist':
        return 'Artist'
      case 'album':
        return 'Album'
      case 'action':
        return 'Action'
      default:
        return 'Search'
    }
  }
</script>
