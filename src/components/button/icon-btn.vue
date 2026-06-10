<template>
  <v-btn :class="sized.box" class="grx-IconButton">
    <svg-sprite :src="src" :class="sized.icon" />
  </v-btn>
</template>

<script setup lang="ts">
import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
import { computed } from 'vue'
import { IconBtnProps } from '@groovex/ui/button/icon-btn.types'

const { size } = defineProps<IconBtnProps>()
const sized = computed(() => {
  const box = size?.box ?? 'md',
    icon = size?.icon ?? 'md'
  return {
    box: box === 'sm' ? 'w-6 h-6' : box === 'lg' ? 'w-12 h-12' : 'w-9 h-9',
    icon: icon === 'sm' ? 'w-5 h-5' : icon === 'lg' ? 'w-9 h-9' : 'w-7 h-7',
  }
})
</script>

<style>
.grx-IconButton {
  min-width: unset !important;
}
</style>
