<template>
  <v-btn
    variant="flat"
    :disabled="disabled || loading"
    :class="[
      'grx-CustomBtn text-none font-semibold text-xs tracking-normal shadow-none cursor-pointer transition-all duration-150',
      variant === 'primary' ? 'grx-CustomBtn-primary' : 'grx-CustomBtn-secondary',
      `grx-CustomBtn-size-${size}`,
      `grx-CustomBtn-rounded-${rounded}`,
      block ? 'w-full' : '',
    ]">
    <template v-slot:default>
      <div class="flex items-center justify-center gap-2 py-0.5">
        <!-- Spinner -->
        <span v-if="loading" class="w-3.5 h-3.5 rounded-full border-2 border-current/20 border-t-current animate-spin shrink-0"></span>
        <slot></slot>
      </div>
    </template>
  </v-btn>
</template>

<script setup lang="ts">
  interface Props {
    variant?: 'primary' | 'secondary'
    disabled?: boolean
    loading?: boolean
    block?: boolean
    rounded?: 'md' | 'lg' | 'full'
    size?: 'sm' | 'md' | 'lg'
  }

  withDefaults(defineProps<Props>(), {
    variant: 'primary',
    disabled: false,
    loading: false,
    block: false,
    rounded: 'lg',
    size: 'md',
  })
</script>

<style>
  .grx-CustomBtn {
    min-width: unset !important;
  }

  /* Sizes */
  .grx-CustomBtn-size-sm {
    height: 28px !important;
    padding: 0 12px !important;
  }

  .grx-CustomBtn-size-md {
    height: 34px !important;
    padding: 0 16px !important;
  }

  .grx-CustomBtn-size-lg {
    height: 40px !important;
    padding: 0 20px !important;
  }

  /* Rounded shapes */
  .grx-CustomBtn-rounded-md {
    border-radius: 6px !important;
  }

  .grx-CustomBtn-rounded-lg {
    border-radius: 8px !important;
  }

  .grx-CustomBtn-rounded-full {
    border-radius: 9999px !important;
  }

  .grx-CustomBtn-primary {
    background-color: var(--color-theme-accent) !important;
    color: var(--color-theme-text-on-accent) !important;
    box-shadow: 0 4px 12px var(--color-theme-accent-glow) !important;
  }

  .grx-CustomBtn-primary:hover:not(:disabled) {
    background-color: var(--color-theme-accent-hover) !important;
    transform: scale(1.02);
  }

  .grx-CustomBtn-primary:active:not(:disabled) {
    transform: scale(0.96);
  }

  .grx-CustomBtn-secondary {
    background-color: transparent !important;
    border: 1px solid var(--color-theme-border) !important;
    color: var(--color-theme-text-muted) !important;
  }

  .grx-CustomBtn-secondary:hover:not(:disabled) {
    border-color: var(--color-theme-border-hover) !important;
    color: var(--color-theme-text-secondary) !important;
    background-color: var(--color-theme-bg-card-hover) !important;
  }

  /* Disabled state overrides */
  .grx-CustomBtn:disabled {
    opacity: 0.5 !important;
    cursor: not-allowed !important;
    pointer-events: none !important;
  }
</style>
