<template>
  <v-btn
    variant="flat"
    class="grx-CustomBtn text-none font-semibold text-xs tracking-normal shadow-none cursor-pointer transition-all duration-150"
    :disabled="disabled || loading">
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
    variant?: 'primary' | 'secondary';
    disabled?: boolean;
    loading?: boolean;
    block?: boolean;
    size?: 'sm' | 'md' | 'lg';
  }

  withDefaults(defineProps<Props>(), {
    variant: 'primary',
    disabled: false,
    loading: false,
    block: false,
    size: 'md',
  });
</script>

<style>
  .grx-CustomBtn {
    min-width: unset !important;
  }
</style>
