<template>
  <button
    @click="togglePlay"
    :class="isPlaying ? 'paused' : 'playing'"
    class="MainController"
    aria-label="Play/Pause"
  >
    <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
      <path class="bar left-bar" d="M6,4 L10,4 L10,20 L6,20 Z" />
      <path class="bar right-bar" d="M14,4 L18,4 L18,20 L14,20 Z" />
    </svg>
  </button>
</template>

<script setup lang="ts">
  const props = withDefaults(
    defineProps<{
      isPlaying?: boolean
    }>(),
    {
      isPlaying: false,
    },
  )

  const emit = defineEmits<{ (e: 'state', state: boolean): void }>()

  function togglePlay() {
    emit('state', !props.isPlaying)
  }
</script>
<style>
  .MainController {
    --size: 32px;
    border: none;
    cursor: pointer;
    padding: 15px;
    border-radius: 50%;
    width: 60px;
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    outline: none;
    transition: background 0.2s;

    svg {
      min-width: var(--size);
      min-height: var(--size);
      width: var(--size);
      height: var(--size);
    }

    .bar {
      fill: #f1f1f1;
      transition: d 0.25s cubic-bezier(0.4, 0, 0.2, 1);
    }

    &.playing {
      .left-bar {
        d: path('M6,4 L12,7.5 L12,16.5 L6,20 Z');
      }

      .right-bar {
        d: path('M12,7.5 L20,12 L20,12 L12,16.5 Z');
      }
    }
  }
</style>
