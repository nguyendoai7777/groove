<template>
  <v-dialog v-model="store.isOpen" max-width="600" attach="body" position="top">
    <v-card class="pa-2">
      <div class="h-10 border-b border-b-gray-400/30 flex items-center justify-center px-3">
        <input @input="onInput" class="w-stretch border-none outline-0 text-xs text-gray-300" />
      </div>
      <OverlayScrollbarsComponent class="max-h-[80svh]" defer :options="OSOptions">
        <div class="h-[300svh]">
          <v-list v-if="items.length > 0" max-height="300">
            <v-list-item
              v-for="item in items"
              :key="item.id"
              :title="item.title"
              :subtitle="item.description"
              @click="selectItem(item)"
            >
              <template #append>
                <kbd v-if="item.shortcut">{{ item.shortcut }}</kbd>
              </template>
            </v-list-item>
          </v-list>
        </div>
      </OverlayScrollbarsComponent>
    </v-card>
  </v-dialog>
</template>

<script setup lang="ts">
  import { reactive } from 'vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { useCommandPaletteStore } from '@groovex/store'
  import type { PartialOptions } from 'overlayscrollbars'

  const store = useCommandPaletteStore()

  const OSOptions: PartialOptions = { scrollbars: { autoHide: 'scroll' } }

  const items = reactive<any[]>([])

  function onInput(e: InputEvent) {
    const target = e.target as HTMLInputElement
    const value = target.value
    if (value.length && value[0] === '/') {
      console.log(`@@ start cmd`)
    } else {
      console.log(`@@ entire search`)
    }
    console.log(`@@e`, e)
  }

  function selectItem(_item: any) {}
</script>
