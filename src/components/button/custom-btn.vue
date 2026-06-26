<template>
  <v-btn
    variant="flat"
    :disabled="disabled || loading"
    :class="[
      'grx-CustomBtn text-none font-semibold text-xs tracking-normal shadow-none cursor-pointer transition-all duration-150',
      variant === 'primary' ? 'grx-CustomBtn-primary' : 'grx-CustomBtn-secondary',
      sizeMapper[size],
      roundedMapper[rounded],
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
  type Rounded = 'xs' | 'sm' | 'md' | 'lg' | 'xl' | 'full';
  interface Props {
    variant?: 'primary' | 'secondary';
    disabled?: boolean;
    loading?: boolean;
    block?: boolean;
    rounded?: Rounded;
    size?: 'sm' | 'md' | 'lg';
  }

  withDefaults(defineProps<Props>(), {
    variant: 'primary',
    disabled: false,
    loading: false,
    block: false,
    rounded: 'lg',
    size: 'md',
  });

  const roundedMapper: Record<Rounded, string> = {
    xs: 'rounded-xs',
    sm: 'rounded-sm',
    md: 'rounded-md',
    lg: 'rounded-lg',
    xl: 'rounded-xl',
    full: 'rounded-full',
  };

  const sizeMapper: Record<NonNullable<Props['size']>, string> = {
    sm: '!h-[28px] !px-3',
    md: '!h-[34px] !px-4',
    lg: '!h-[40px] !px-5',
  };
</script>

<style>
  .grx-CustomBtn {
    min-width: unset !important;
  }

  .grx-CustomBtn-primary {
    background-color: var(--color-theme-accent) !important;
    color: var(--color-theme-text-on-accent) !important;
    box-shadow: 0 4px 12px var(--color-theme-accent-glow) !important;
  }

  .grx-CustomBtn-primary:hover:not(:disabled) {
    background-color: var(--color-theme-accent-hover) !important;
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
