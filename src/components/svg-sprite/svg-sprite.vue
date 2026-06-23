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
    VMin: '/icons/volume.svg#VMin',
    VMedium: '/icons/volume.svg#VMedium',
    VMax: '/icons/volume.svg#VMax',

    AngleLeft: '/icons/arrow.svg#AngleLeft',
    AngleRight: '/icons/arrow.svg#AngleRight',
    ArrowsExpand: '/icons/arrow.svg#ArrowsExpand',
    ArrowsCompress: '/icons/arrow.svg#ArrowsCompress',

    Close: '/icons/action.svg#Close',
    Minus: '/icons/action.svg#Minus',
    Settings: '/icons/action.svg#Settings',
    Play: '/icons/action.svg#Play',
    Folder: '/icons/action.svg#Folder',
    LyricsBubble: '/icons/action.svg#LyricsBubble',
    HelpCircle: '/icons/action.svg#HelpCircle',
    Delete: '/icons/action.svg#Delete',
    Edit: '/icons/action.svg#Edit',

    Command: '/icons/cli.svg#Command',
    Song: '/icons/cli.svg#Song',
    Artist: '/icons/cli.svg#Artist',
    Album: '/icons/cli.svg#Album',
    Action: '/icons/cli.svg#Action',
    Search: '/icons/cli.svg#Search',

    Shuffle: '/icons/media-ctrl.svg#Shuffle',
    Loop: '/icons/media-ctrl.svg#Loop',
    LoopBadge: '/icons/media-ctrl.svg#LoopBadge',
    Next: '/icons/media-ctrl.svg#Next',
    Prev: '/icons/media-ctrl.svg#Prev',
    NowPlaying: '/icons/media-ctrl.svg#NowPlaying',
  }
</script>
