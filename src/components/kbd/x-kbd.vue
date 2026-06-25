<template>
  <kbd
    class="font-mono border rounded transition-colors duration-150"
    :class="[
      size === 'sm' ? 'text-[9px] px-1 py-0.25' : 'text-[10px] px-1.5 py-0.5',
      active
        ? 'bg-(--cmd-kbd-bg-active) text-(--cmd-kbd-text-active) border-(--cmd-kbd-border-active)'
        : 'bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border-(--cmd-kbd-border) shadow-kbd-bottom',
    ]">
    <slot></slot>
  </kbd>
</template>

<script setup lang="ts">
  interface Props {
    active?: boolean;
    size?: 'sm' | 'md';
  }

  withDefaults(defineProps<Props>(), {
    active: false,
    size: 'sm',
  });
</script>
