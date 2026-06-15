<template>
  <svg :class="className ?? ''">
    <use :href="sourceMap[src as SVGSrc]" />
  </svg>
</template>

<script setup lang="ts">
  import { SVGSpriteProps, SVGSrc } from '@groovex/ui/svg-sprite/svg-sprite.types.ts'

  const { src } = defineProps<SVGSpriteProps>()

  const sourceMap: Record<SVGSrc, string> = {
    VMute: '/icons/volume.svg#VMute',

    AngleLeft: '/icons/arrow.svg#AngleLeft',
    AngleRight: '/icons/arrow.svg#AngleRight',
    ArrowsExpand: '/icons/arrow.svg#ArrowsExpand',
    ArrowsCompress: '/icons/arrow.svg#ArrowsCompress',

    Close: '/icons/action.svg#Close',
    Minus: '/icons/action.svg#Minus',

    Command: '/icons/cli.svg#Command',
    Song: '/icons/cli.svg#Song',
    Artist: '/icons/cli.svg#Artist',
    Album: '/icons/cli.svg#Album',
    Action: '/icons/cli.svg#Action',
    Search: '/icons/cli.svg#Search',
  }
</script>
