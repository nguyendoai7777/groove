<template>
  <svg
    class="playing-indicator"
    :class="{ 'playing-indicator-paused': paused }"
    viewBox="0 0 14 14"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
  >
    <rect class="bar bar-1" x="1.5" y="2" width="2" height="10" rx="1" fill="currentColor" />
    <rect class="bar bar-2" x="4.5" y="2" width="2" height="10" rx="1" fill="currentColor" />
    <rect class="bar bar-3" x="7.5" y="2" width="2" height="10" rx="1" fill="currentColor" />
    <rect class="bar bar-4" x="10.5" y="2" width="2" height="10" rx="1" fill="currentColor" />
  </svg>
</template>

<script setup lang="ts">
  defineProps<{
    paused: boolean
  }>()
</script>

<style>
  .playing-indicator {
    width: 14px;
    height: 14px;
    display: flex;
    align-items: flex-end;
    color: var(--grx-visualizer-color, var(--color-cyan-400, #22d3ee));
  }

  .playing-indicator .bar {
    transform-origin: bottom;
  }

  .playing-indicator-paused .bar {
    animation-play-state: paused !important;
  }

  .playing-indicator .bar-1 {
    animation: bounce-bar-1 0.3s ease-in-out infinite alternate;
  }

  .playing-indicator .bar-2 {
    animation: bounce-bar-2 0.38s ease-in-out infinite alternate;
    animation-delay: 0.075s;
  }

  .playing-indicator .bar-3 {
    animation: bounce-bar-3 0.28s ease-in-out infinite alternate;
    animation-delay: 0.025s;
  }

  .playing-indicator .bar-4 {
    animation: bounce-bar-4 0.34s ease-in-out infinite alternate;
    animation-delay: 0.125s;
  }

  @keyframes bounce-bar-1 {
    0% {
      transform: scaleY(0.25);
    }
    100% {
      transform: scaleY(0.8);
    }
  }

  @keyframes bounce-bar-2 {
    0% {
      transform: scaleY(0.15);
    }
    100% {
      transform: scaleY(1);
    }
  }

  @keyframes bounce-bar-3 {
    0% {
      transform: scaleY(0.3);
    }
    100% {
      transform: scaleY(0.7);
    }
  }

  @keyframes bounce-bar-4 {
    0% {
      transform: scaleY(0.2);
    }
    100% {
      transform: scaleY(0.9);
    }
  }
</style>
