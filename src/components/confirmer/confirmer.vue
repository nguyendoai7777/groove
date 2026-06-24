<template>
  <v-dialog v-model="dialogVisible" max-width="400">
    <v-card class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
      <v-card-title class="text-md! font-bold! border-b border-theme-border/80 px-4py-3 v-card-title text-md! px-4 py-3">
        {{ title }}
      </v-card-title>

      <v-card-text class="px-6! py-5! text-sm! text-theme-text-muted">
        {{ content }}
      </v-card-text>

      <v-card-actions class="px-4 py-3 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
        <custom-btn variant="secondary" @click="onCancel">Cancel</custom-btn>
        <custom-btn variant="primary" @click="onOk">OK</custom-btn>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { computed } from 'vue';
  import CustomBtn from '@groovex/ui/button/custom-btn.vue';

  const props = defineProps<{
    modelValue: boolean;
    title: string;
    content: string;
  }>();

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void;
    (e: 'ok'): void;
    (e: 'cancel'): void;
  }>();

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  });

  function onCancel() {
    dialogVisible.value = false;
    emit('cancel');
  }

  function onOk() {
    dialogVisible.value = false;
    emit('ok');
  }
</script>

<style>
  .grx-ConfirmerCard {
    background-color: var(--color-theme-bg-item) !important;
    color: var(--color-theme-text) !important;
    border: 1px solid var(--color-theme-border) !important;
    border-radius: 12px !important;
  }
</style>
