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
        <span v-if="loading" class="w-3.5 h-3.5 rounded-full border-2 border-white/20 border-t-white animate-spin shrink-0"></span>
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

<style scoped>
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
    background-color: #0891b2 !important;
    color: #ffffff !important;
    box-shadow: 0 4px 12px rgba(6, 182, 212, 0.15) !important;
  }

  .grx-CustomBtn-primary:hover:not(:disabled) {
    background-color: #06b6d4 !important;
    transform: scale(1.02);
  }

  .grx-CustomBtn-primary:active:not(:disabled) {
    transform: scale(0.96);
  }

  .grx-CustomBtn-secondary {
    background-color: transparent !important;
    border: 1px solid #27272a !important;
    color: #a1a1aa !important;
  }

  .grx-CustomBtn-secondary:hover:not(:disabled) {
    border-color: #3f3f46 !important;
    color: #e4e4e7 !important;
    background-color: rgba(39, 39, 42, 0.4) !important;
  }

  /* Disabled state overrides */
  .grx-CustomBtn:disabled {
    opacity: 0.5 !important;
    cursor: not-allowed !important;
    pointer-events: none !important;
  }
</style>
