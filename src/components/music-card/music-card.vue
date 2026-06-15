<template>
  <div
    class="music-card group relative flex flex-col p-3 rounded-xl transition-all duration-300 border-2 border-transparent bg-zinc-900/20 hover:bg-zinc-900/60 cursor-pointer"
    :style="{
      '--glow-color': glowColor,
      '--border-glow': borderGlowColor,
    }"
    @click="$emit('click')"
  >
    <!-- Thumbnail Container -->
    <div class="relative w-full aspect-square rounded-lg overflow-hidden bg-zinc-800 shadow-md">
      <img
        v-if="thumbnail"
        :src="thumbnail"
        :alt="title"
        class="w-full h-full object-cover select-none pointer-events-none transition-transform duration-500 group-hover:scale-105"
      />
      <!-- Fallback Placeholder -->
      <div
        v-else
        class="w-full h-full flex items-center justify-center select-none"
        :class="placeholderClass"
      >
        <svg-sprite
          :src="type === 'folder' ? 'Play' : 'Album'"
          :class="
            type === 'folder' ? 'w-16 h-16 text-white/90 drop-shadow-lg' : 'w-16 h-16 text-zinc-600'
          "
        />
      </div>

      <!-- Hover Overlay -->
      <div
        class="absolute inset-0 bg-black/40 backdrop-blur-[2px] opacity-0 group-hover:opacity-100 flex items-center justify-center gap-4 transition-all duration-300"
      >
        <!-- Play Button -->
        <div class="relative group/btn">
          <button
            class="w-12 h-12 rounded-full bg-zinc-900/80 hover:bg-zinc-800 border border-white/10 hover:border-white/20 text-white flex items-center justify-center transition-all duration-200 hover:scale-110 shadow-lg cursor-pointer"
            @click.stop="$emit('play', { type, title })"
          >
            <!-- Offset play icon slightly to the right for optical center alignment -->
            <svg-sprite src="Play" class="w-5 h-5 translate-x-[1px]" />
          </button>
          <span
            class="absolute bottom-14 left-1/2 -translate-x-1/2 bg-zinc-950/90 text-zinc-100 text-[11px] px-2 py-1 rounded-md border border-zinc-800/80 shadow-md opacity-0 group-hover/btn:opacity-100 transition-opacity duration-150 whitespace-nowrap pointer-events-none"
          >
            Play all
          </span>
        </div>

        <!-- Settings Button -->
        <div class="relative group/btn">
          <button
            class="w-12 h-12 rounded-full bg-zinc-900/80 hover:bg-zinc-800 border border-white/10 hover:border-white/20 text-white flex items-center justify-center transition-all duration-200 hover:scale-110 shadow-lg cursor-pointer"
            @click.stop="$emit('settings', { type, title })"
          >
            <svg-sprite src="Settings" class="w-5 h-5" />
          </button>
          <span
            class="absolute bottom-14 left-1/2 -translate-x-1/2 bg-zinc-950/90 text-zinc-100 text-[11px] px-2 py-1 rounded-md border border-zinc-800/80 shadow-md opacity-0 group-hover/btn:opacity-100 transition-opacity duration-150 whitespace-nowrap pointer-events-none"
          >
            Settings
          </span>
        </div>
      </div>
    </div>

    <!-- Metadata Details -->
    <div class="mt-3 flex flex-col min-w-0">
      <span
        class="text-sm font-semibold truncate text-zinc-100 group-hover:text-white transition-colors duration-150"
      >
        {{ title }}
      </span>
      <span class="text-xs text-zinc-400 mt-1 truncate">
        {{ displaySubtitle }}
      </span>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed } from 'vue'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import { MusicCardProps } from '@groovex/ui/music-card/music-card.types.ts'

  const props = withDefaults(defineProps<MusicCardProps>(), {
    songsCount: 0,
  })

  defineEmits<{
    (e: 'click'): void
    (e: 'play', payload: { type: 'album' | 'folder'; title: string }): void
    (e: 'settings', payload: { type: 'album' | 'folder'; title: string }): void
  }>()

  // Generate border glow and shadow color based on accentColor
  const borderGlowColor = computed(() => {
    return props.accentColor || '#06b6d4' // defaults to cyan
  })

  const glowColor = computed(() => {
    const baseColor = props.accentColor || '#06b6d4'
    // If it's hex, add opacity
    if (baseColor.startsWith('#')) {
      return `${baseColor}40` // 25% opacity
    }
    if (baseColor.startsWith('rgb')) {
      return baseColor.replace('rgb', 'rgba').replace(')', ', 0.25)')
    }
    return baseColor
  })

  // Display subtitle based on props
  const displaySubtitle = computed(() => {
    if (props.subtitle) return props.subtitle
    if (props.type === 'folder') {
      return props.songsCount > 0 ? `${props.songsCount} songs` : 'Folder'
    }
    return 'Unknown Artist'
  })

  // Dynamic classes for placeholder background
  const placeholderClass = computed(() => {
    if (props.type === 'folder') {
      return 'bg-gradient-to-br from-cyan-500 to-blue-600 shadow-[inset_0_2px_10px_rgba(255,255,255,0.15)]'
    }
    return 'bg-zinc-800 border border-zinc-700/50'
  })
</script>

<style scoped>
  .music-card:hover {
    border-color: var(--border-glow) !important;
    box-shadow: 0 0 20px var(--glow-color) !important;
  }
</style>
