<template>
  <div class="h-(--nav-head-h) flex items-center relative">
    <icon-btn src="AngleLeft" />
    <icon-btn title="Minimize" :size="{ icon: 'sm' }" src="Minus" class="ml-auto" />
    <icon-btn title="Restore" :size="{ icon: 'sm' }" src="ArrowsCompress" />
    <icon-btn title="Maximize" :size="{ icon: 'sm' }" src="ArrowsExpand" />
    <icon-btn title="Close" :size="{ icon: 'sm' }" src="Close" />
    <div class="absolute top-1/2 left-1/2 -translate-1/2">Search all</div>
  </div>
</template>

<script setup>
import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
import IconBtn from '@groovex/ui/button/icon-btn.vue'
</script>
