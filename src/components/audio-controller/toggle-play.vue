<template>
  <button @click="togglePlay" :class="isPlaying ? 'paused' : 'playing'" class="grx-TogglePlay MainController" aria-label="Play/Pause">
    <span class="icon" aria-hidden="true">
      <span class="bar left-bar"></span>
      <span class="bar right-bar"></span>
    </span>
  </button>
</template>

<script setup lang="ts">
  const props = withDefaults(
    defineProps<{
      isPlaying?: boolean;
    }>(),
    {
      isPlaying: false,
    },
  );

  const emit = defineEmits<{ (e: 'state', state: boolean): void }>();

  function togglePlay() {
    emit('state', !props.isPlaying);
  }
</script>
<style>
  /* The two bars morph into a triangle with `clip-path`, not with the CSS `d`
     property: `d` is Blink-only, so on WebKit (macOS) the paths never moved and
     the button stayed stuck on the pause bars. Both halves keep four points in
     percentages so the polygons interpolate everywhere. */
  .MainController {
    --size: 32px;
    /* Coordinates below are the old 24x24 viewBox expressed as percentages. */
    --bar-top: 16.667%;
    --bar-bottom: 83.333%;
    --tri-left: 25%;
    --tri-mid: 50%;
    --tri-right: 83.333%;
    --tri-mid-top: 30.952%;
    --tri-mid-bottom: 69.048%;
    border: none;
    cursor: pointer;
    padding: 15px;
    border-radius: 50%;
    width: 60px;
    height: 60px;
    display: flex;
    align-items: center;
    justify-content: center;
    outline: none;
    transition: background 0.2s;

    .icon {
      position: relative;
      display: block;
      min-width: var(--size);
      min-height: var(--size);
      width: var(--size);
      height: var(--size);
    }

    .bar {
      position: absolute;
      inset: 0;
      background-color: currentColor;
      transition:
        clip-path 0.25s cubic-bezier(0.4, 0, 0.2, 1),
        -webkit-clip-path 0.25s cubic-bezier(0.4, 0, 0.2, 1);
    }

    /* Paused icon: two vertical bars (x 6-10 and 14-18 of the old viewBox). */
    .left-bar {
      -webkit-clip-path: polygon(
        var(--tri-left) var(--bar-top),
        41.667% var(--bar-top),
        41.667% var(--bar-bottom),
        var(--tri-left) var(--bar-bottom)
      );
      clip-path: polygon(
        var(--tri-left) var(--bar-top),
        41.667% var(--bar-top),
        41.667% var(--bar-bottom),
        var(--tri-left) var(--bar-bottom)
      );
    }

    .right-bar {
      -webkit-clip-path: polygon(58.333% var(--bar-top), 75% var(--bar-top), 75% var(--bar-bottom), 58.333% var(--bar-bottom));
      clip-path: polygon(58.333% var(--bar-top), 75% var(--bar-top), 75% var(--bar-bottom), 58.333% var(--bar-bottom));
    }

    /* Play icon: the same two shapes collapsed into the halves of a triangle,
       split down the middle. The apex is a doubled point so the vertex count
       matches the bars and the transition can interpolate. */
    &.playing {
      .left-bar {
        -webkit-clip-path: polygon(
          var(--tri-left) var(--bar-top),
          var(--tri-mid) var(--tri-mid-top),
          var(--tri-mid) var(--tri-mid-bottom),
          var(--tri-left) var(--bar-bottom)
        );
        clip-path: polygon(
          var(--tri-left) var(--bar-top),
          var(--tri-mid) var(--tri-mid-top),
          var(--tri-mid) var(--tri-mid-bottom),
          var(--tri-left) var(--bar-bottom)
        );
      }

      .right-bar {
        -webkit-clip-path: polygon(
          var(--tri-mid) var(--tri-mid-top),
          var(--tri-right) var(--tri-mid),
          var(--tri-right) var(--tri-mid),
          var(--tri-mid) var(--tri-mid-bottom)
        );
        clip-path: polygon(
          var(--tri-mid) var(--tri-mid-top),
          var(--tri-right) var(--tri-mid),
          var(--tri-right) var(--tri-mid),
          var(--tri-mid) var(--tri-mid-bottom)
        );
      }
    }
  }
</style>
