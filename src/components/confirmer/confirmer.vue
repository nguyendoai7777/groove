<template>
  <v-dialog v-model="dialogVisible" max-width="400">
    <v-card
      class="grx-ConfirmerCard bg-zinc-900! text-zinc-100! border border-zinc-800! rounded-xl! overflow-hidden shadow-2xl"
    >
      <v-card-title class="text-md! font-bold! border-b border-zinc-800/80 px-6 py-4 text-white">
        {{ title }}
      </v-card-title>

      <v-card-text class="px-6! py-5! text-sm! text-zinc-400">
        {{ content }}
      </v-card-text>

      <v-card-actions
        class="px-6 py-4 flex justify-end gap-2 bg-zinc-950/20 border-t border-zinc-800/50"
      >
        <button
          class="px-4 py-2 text-xs font-semibold rounded-md border border-zinc-800 hover:border-zinc-700 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-all cursor-pointer"
          @click="onCancel"
        >
          Cancel
        </button>
        <button
          class="px-4 py-2 text-xs font-semibold rounded-md bg-cyan-600 hover:bg-cyan-500 text-white transition-all shadow-md cursor-pointer"
          @click="onOk"
        >
          OK
        </button>
      </v-card-actions>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { computed } from 'vue'

  const props = defineProps<{
    modelValue: boolean
    title: string
    content: string
  }>()

  const emit = defineEmits<{
    (e: 'update:modelValue', value: boolean): void
    (e: 'ok'): void
    (e: 'cancel'): void
  }>()

  const dialogVisible = computed({
    get: () => props.modelValue,
    set: (val) => emit('update:modelValue', val),
  })

  function onCancel() {
    dialogVisible.value = false
    emit('cancel')
  }

  function onOk() {
    dialogVisible.value = false
    emit('ok')
  }
</script>

<style>
  .grx-ConfirmerCard {
    background-color: #18181b !important;
    color: #f4f4f5 !important;
    border: 1px solid #27272a !important;
    border-radius: 12px !important;
  }
</style>
