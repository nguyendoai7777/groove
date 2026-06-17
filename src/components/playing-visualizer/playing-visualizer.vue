<template>
  <svg
    class="PlayingIndicator"
    :class="{ PlayingIndicatorPaused: paused }"
    viewBox="0 0 14 14"
    fill="none"
    xmlns="http://www.w3.org/2000/svg">
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
  .PlayingIndicator {
    --size: 14px;
    width: var(--size);
    height: var(--size);
    display: flex;
    align-items: flex-end;
    color: var(--deco-visualizer-color, var(--color-cyan-400, #22d3ee));
    .bar {
      transform-origin: bottom;
    }
    .bar-1 {
      animation: bounce-bar-1 0.3s ease-in-out infinite alternate;
    }
    .bar-2 {
      animation: bounce-bar-2 0.38s ease-in-out infinite alternate;
      animation-delay: 0.075s;
    }
    .bar-3 {
      animation: bounce-bar-3 0.28s ease-in-out infinite alternate;
      animation-delay: 0.025s;
    }
    .bar-4 {
      animation: bounce-bar-4 0.34s ease-in-out infinite alternate;
      animation-delay: 0.125s;
    }
  }
  .PlayingIndicatorPaused .bar {
    animation-play-state: paused !important;
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
