<template>
  <div class="grx-SettingGeneral flex flex-col gap-4">
    <div class="grid grid-cols-2 gap-3">
      <label>
        <div class="block mb-2 text-theme-text-secondary font-medium text-xs tracking-wide">Audio Seek Step (seconds)</div>
        <v-text-field
          v-model.number="seekStep"
          type="number"
          min="1"
          placeholder="5"
          density="compact"
          variant="outlined"
          color="cyan-accent-3"
          hide-details />
      </label>

      <label>
        <div class="block mb-2 text-theme-text-secondary font-medium text-xs tracking-wide">Volume Adjust Step</div>
        <v-text-field
          v-model.number="volumeStep"
          type="number"
          min="1"
          max="100"
          placeholder="2"
          density="compact"
          variant="outlined"
          color="cyan-accent-3"
          hide-details />
      </label>
    </div>

    <div class="border-t border-theme-border/30 pt-4 mt-2">
      <div class="flex justify-between items-center mb-3">
        <div class="text-theme-text-secondary font-medium text-xs tracking-wide">Equalizer</div>
        <div class="flex items-center gap-1.5">
          <span class="text-[11px] text-theme-text-muted">Preset:</span>
          <v-menu>
            <template #activator="{ props }">
              <v-btn
                v-bind="props"
                variant="outlined"
                density="compact"
                class="text-xs w-[100px] font-semibold border-theme-border! text-theme-text-secondary!">
                {{ currentPresetName }}
              </v-btn>
            </template>
            <v-list class="bg-theme-bg-item border border-theme-border! text-xs" density="compact">
              <v-list-item
                v-for="preset in EQ_PRESETS"
                color="cyan-accent-3"
                class="cursor-pointer hover:bg-theme-bg-placeholder/20 px-3"
                :key="preset.name"
                :active="currentPresetName === preset.name"
                @click="applyPreset(preset)">
                <v-list-item-title class="text-xs">{{ preset.name }}</v-list-item-title>
              </v-list-item>
            </v-list>
          </v-menu>
        </div>
      </div>

      <!-- Bass Booster Special Controls -->
      <div class="bg-theme-bg-placeholder/5 border border-theme-border/30 rounded-lg p-3 mb-4 flex items-center justify-between">
        <div class="flex flex-col gap-0.5">
          <span class="text-xs font-semibold text-white flex items-center gap-1">
            <span class="w-1.5 h-1.5 rounded-full bg-cyan-accent-3 animate-pulse"></span>
            Bass Boost (Tăng Siêu Trầm)
          </span>
          <span class="text-[10px] text-theme-text-muted">Tăng cường riêng dải âm trầm sâu cho trải nghiệm Bass tốt nhất</span>
        </div>
        <div class="flex items-center gap-3 w-44 pr-2">
          <v-slider
            hide-details
            color="cyan-accent-3"
            class="cursor-pointer w-full"
            v-model="bassBoost"
            :min="0"
            :max="10"
            :step="1"
            :track-size="2"
            :thumb-size="10"
            @update:model-value="onBassBoostChange" />
          <span class="text-xs font-mono w-7 text-right text-cyan-accent-3 font-semibold">+{{ bassBoost }}dB</span>
        </div>
      </div>

      <!-- 10-Band EQ Panel -->
      <div class="flex justify-between items-stretch bg-theme-bg-placeholder/5 border border-theme-border/20 rounded-xl h-fit relative">
        <!-- Visualizer Curve Line -->
        <svg
          class="absolute pointer-events-none z-0"
          style="left: 0; right: 0; width: 100%; top: 20px; height: 300px"
          viewBox="0 0 1000 100"
          preserveAspectRatio="none">
          <!-- Glow path -->
          <path
            :d="curvePath"
            vector-effect="non-scaling-stroke"
            fill="none"
            stroke="rgba(6, 182, 212, 0.25)"
            stroke-width="3"
            stroke-linecap="round" />
          <!-- Main solid path -->
          <path
            :d="curvePath"
            vector-effect="non-scaling-stroke"
            fill="none"
            stroke="rgb(34, 211, 238)"
            stroke-width="1.2"
            stroke-linecap="round" />
        </svg>
        <div v-for="(band, idx) in OCTAVE_BANDS" :key="band" class="flex flex-col items-center flex-1 h-fit min-w-0 z-10">
          <!-- DB value display -->
          <span class="text-[9px] font-mono text-theme-text-secondary select-none mb-1">
            {{ getDisplayGain(idx) }}
          </span>
          <!-- Slider -->
          <div class="grow flex justify-center py-1">
            <v-slider
              direction="vertical"
              min="-10"
              max="10"
              step="0.5"
              hide-details
              color="cyan-accent-3"
              track-color="rgba(255, 255, 255, 0.1)"
              class="cursor-pointer h-full"
              v-model="eqGains[idx]"
              :thumb-size="12"
              :track-size="2"
              @update:model-value="onSliderChange" />
          </div>
          <!-- Band Label -->
          <span class="text-[9px] text-theme-text-muted select-none mt-1 font-medium whitespace-nowrap">
            {{ formatBandLabel(band) }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
  import { computed } from 'vue';
  import { storeToRefs } from 'pinia';
  import { useAudioPlayer } from '@groovex/store';
  import { OCTAVE_BANDS } from '@groovex/core';

  const player = useAudioPlayer();
  const seekStep = storeToRefs(player).seekStep;
  const volumeStep = storeToRefs(player).volumeStep;
  const eqGains = storeToRefs(player).eqGains;
  const bassBoost = storeToRefs(player).bassBoost;
  const currentPresetName = storeToRefs(player).currentPresetName;

  interface EQPreset {
    name: string;
    gains: number[];
  }

  const EQ_PRESETS: EQPreset[] = [
    { name: 'Custom', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: 'Flat', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: 'Bass Strong', gains: [6, 5, 3, 1, 0, 0, 0, 0, 0, 0] },
    { name: 'Bass Moderate', gains: [4, 3, 2, 1, 0, 0, 0, 0, 0, 0] },
    { name: 'Pop', gains: [-1.5, -1, 0, 2, 4, 4, 2, 0, -1, -1.5] },
    { name: 'Rock', gains: [4, 3, -1.5, -2.5, -1, 1, 3.5, 4.5, 5, 5] },
    { name: 'Electronic', gains: [5, 4, 1, 0, -1, 2, 3, 4, 5, 5] },
    { name: 'Vocal', gains: [-3, -2, -1, 1, 3, 4, 3, 2, 1, 0] },
    { name: 'Treble', gains: [0, 0, 0, 0, 0, 1, 3, 5, 7, 9] },
  ];

  function applyPreset(preset: EQPreset) {
    currentPresetName.value = preset.name;
    eqGains.value = [...preset.gains];
  }

  function onSliderChange() {
    currentPresetName.value = 'Custom';
  }

  function onBassBoostChange() {
    if (currentPresetName.value !== 'Custom' && !currentPresetName.value.includes('Bass')) {
      currentPresetName.value = 'Custom';
    }
  }

  function getDisplayGain(idx: number): string {
    const gain = eqGains.value[idx];
    return gain > 0 ? `+${gain}` : `${gain}`;
  }

  interface Point {
    x: number;
    y: number;
  }

  // Catmull-Rom spline interpolation to SVG Cubic Bezier path
  function catmullRom2bezier(points: Point[]): string {
    if (points.length < 2) return '';
    let d = `M ${points[0].x} ${points[0].y}`;
    for (let i = 0; i < points.length - 1; i++) {
      const p0 = points[i - 1] || points[0];
      const p1 = points[i];
      const p2 = points[i + 1];
      const p3 = points[i + 2] || p2;

      const cp1x = p1.x + (p2.x - p0.x) / 6;
      const cp1y = p1.y + (p2.y - p0.y) / 6;

      const cp2x = p2.x - (p3.x - p1.x) / 6;
      const cp2y = p2.y - (p3.y - p1.y) / 6;

      d += ` C ${cp1x.toFixed(1)} ${cp1y.toFixed(1)}, ${cp2x.toFixed(1)} ${cp2y.toFixed(1)}, ${p2.x.toFixed(1)} ${p2.y.toFixed(1)}`;
    }
    return d;
  }

  const curvePath = computed(() => {
    // Generate coordinate points for each of the 10 bands
    // Horizontal center for column i in 1000px wide viewBox: x_i = i * 100 + 50
    // Vertical value: eqGains value goes from -10 to +10.
    // Map -10 to 10 dB to vertical viewport range [2, 99.5] where 2 is top (+10dB) and 99.5 is bottom (-10dB)
    const points = eqGains.value.map((gain: number, idx: number) => {
      const x = idx * 100 + 50;
      const percentage = (10 - gain) / 20; // 0 at +10dB, 1 at -10dB
      const y = 2 + percentage * 97.5;
      return { x, y };
    });
    return catmullRom2bezier(points);
  });

  function formatBandLabel(band: number): string {
    if (band >= 1000) {
      return `${band / 1000}k`;
    }
    return `${band}`;
  }
</script>
