<template>
  <div class="min-w-57.5 bg-(--nav-bg) flex flex-col" style="backdrop-filter: blur(23px); height: calc(100svh - var(--audio-controller-h))">
    <div
      class="h-(--nav-head-h) duration-200 hover:bg-side-nav-header-hover flex items-center px-2 select-none font-light text-xs cursor-default shrink-0"
      data-tauri-drag-region>
      {{ appInfo.name }}
    </div>

    <div class="flex-1 flex flex-col overflow-y-auto">
      <RouterLink
        v-for="nav in APP_ROUTES[0].children.filter((c) => c.meta && c.meta.title)"
        :key="nav.path"
        :to="'/' + nav.path"
        v-slot="{ isActive }">
        <div
          class="group border-l-[3px] transition-all duration-200"
          :class="
            isActive
              ? 'border-theme-accent-light bg-theme-bg-placeholder/30'
              : 'border-transparent hover:border-theme-border-hover/70 hover:bg-theme-bg-placeholder/10'
          ">
          <v-btn
            class="w-full flex justify-start items-center px-4 text-sm h-(--nav-link-h) shadow-none bg-transparent rounded-none text-left">
            <svg-sprite
              class="w-4 h-4 mr-3"
              :src="getIconForRoute(nav.path)"
              :class="isActive ? 'text-theme-accent-light' : 'text-theme-text-muted group-hover:text-theme-text-secondary'" />
            <span :class="isActive ? 'text-white font-semibold' : 'text-theme-text-muted group-hover:text-theme-text-secondary'">
              {{ nav.meta['title'] }}
            </span>
          </v-btn>
        </div>
      </RouterLink>
    </div>

    <div class="border-t border-theme-border/30 shrink-0">
      <v-btn
        @click="showSettings = true"
        class="w-full flex justify-start items-center px-4 text-sm h-(--nav-link-h) shadow-none bg-transparent hover:bg-theme-bg-placeholder/10 rounded-none text-left text-theme-text-muted hover:text-theme-text-secondary border-l-[3px] border-transparent hover:border-theme-border-hover/70 transition-all duration-200">
        <svg-sprite src="Settings" class="w-4 h-4 mr-3" />
        <span>Settings</span>
      </v-btn>
    </div>

    <!-- Settings Dialog -->
    <v-dialog v-model="showSettings" max-width="800">
      <v-card
        class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
        <v-card-title class="text-md! font-bold! border-b border-theme-border/80 px-6 py-4 text-white">Settings</v-card-title>
        <v-card-text class="px-1">
          <OverlayScrollbarsComponent :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="px-6 py-5 max-h-[70svh]">
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
                        :key="preset.name"
                        @click="applyPreset(preset)"
                        :active="currentPresetName === preset.name"
                        color="cyan-accent-3"
                        class="cursor-pointer hover:bg-theme-bg-placeholder/20 px-3">
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
                    v-model="bassBoost"
                    :min="0"
                    :max="10"
                    :step="1"
                    :track-size="2"
                    :thumb-size="10"
                    hide-details
                    color="cyan-accent-3"
                    class="cursor-pointer w-full"
                    @update:model-value="onBassBoostChange" />
                  <span class="text-xs font-mono w-7 text-right text-cyan-accent-3 font-semibold">+{{ bassBoost }}dB</span>
                </div>
              </div>

              <!-- 10-Band EQ Panel -->
              <div
                class="flex justify-between items-stretch bg-theme-bg-placeholder/5 border border-theme-border/20 rounded-xl p-3 gap-1 h-fit">
                <div v-for="(band, idx) in OCTAVE_BANDS" :key="band" class="flex flex-col items-center flex-1 h-full min-w-0">
                  <!-- DB value display -->
                  <span class="text-[9px] font-mono text-theme-text-secondary select-none mb-1">
                    {{ getDisplayGain(idx) }}
                  </span>
                  <!-- Slider -->
                  <div class="grow flex justify-center py-1">
                    <v-slider
                      v-model="eqGains[idx]"
                      direction="vertical"
                      min="-10"
                      max="10"
                      step="0.5"
                      hide-details
                      color="cyan-accent-3"
                      track-color="rgba(255, 255, 255, 0.1)"
                      class="cursor-pointer"
                      @update:model-value="onSliderChange" />
                  </div>
                  <!-- Band Label -->
                  <span class="text-[9px] text-theme-text-muted select-none mt-1 font-medium whitespace-nowrap">
                    {{ formatBandLabel(band) }}
                  </span>
                </div>
              </div>
            </div>
          </OverlayScrollbarsComponent>
        </v-card-text>
        <v-card-actions class="px-6 py-4 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
          <custom-btn variant="secondary" @click="cancelSettings">Cancel</custom-btn>
          <custom-btn variant="primary" @click="saveSettings">OK</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup>
  import { ref, watch } from 'vue'
  import { storeToRefs } from 'pinia'
  import { useAudioPlayer } from '@groovex/state'
  import { OCTAVE_BANDS } from '@groovex/core'
  import appInfo from '../../../app.json' with { type: 'json' }
  import { APP_ROUTES } from '../../app.route.ts'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'

  const showSettings = ref(false)
  const player = useAudioPlayer()
  const { seekStep, volumeStep, eqGains, bassBoost, currentPresetName } = storeToRefs(player)

  // Backup variables for OK/Cancel transactions
  let originalSeekStep = 5
  let originalVolumeStep = 2
  let originalEqGains = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  let originalBassBoost = 0
  let originalPresetName = 'Flat'

  // Backup values when the settings modal opens
  watch(showSettings, (open) => {
    if (open) {
      originalSeekStep = seekStep.value
      originalVolumeStep = volumeStep.value
      originalEqGains = [...eqGains.value]
      originalBassBoost = bassBoost.value
      originalPresetName = currentPresetName.value
    }
  })

  const saveSettings = () => {
    showSettings.value = false
  }

  const cancelSettings = () => {
    seekStep.value = originalSeekStep
    volumeStep.value = originalVolumeStep
    eqGains.value = [...originalEqGains]
    bassBoost.value = originalBassBoost
    currentPresetName.value = originalPresetName
    showSettings.value = false
  }

  const EQ_PRESETS = [
    { name: 'Custom', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: 'Flat', gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
    { name: 'Bass Strong', gains: [6, 5, 3, 1, 0, 0, 0, 0, 0, 0] },
    { name: 'Bass Moderate', gains: [4, 3, 2, 1, 0, 0, 0, 0, 0, 0] },
    { name: 'Pop', gains: [-1.5, -1, 0, 2, 4, 4, 2, 0, -1, -1.5] },
    { name: 'Rock', gains: [4, 3, -1.5, -2.5, -1, 1, 3.5, 4.5, 5, 5] },
    { name: 'Electronic', gains: [5, 4, 1, 0, -1, 2, 3, 4, 5, 5] },
    { name: 'Vocal', gains: [-3, -2, -1, 1, 3, 4, 3, 2, 1, 0] },
    { name: 'Treble', gains: [0, 0, 0, 0, 0, 1, 3, 5, 7, 9] },
  ]

  const applyPreset = (preset) => {
    currentPresetName.value = preset.name
    eqGains.value = [...preset.gains]
  }

  const onSliderChange = () => {
    currentPresetName.value = 'Custom'
  }

  const onBassBoostChange = () => {
    if (currentPresetName.value !== 'Custom' && !currentPresetName.value.includes('Bass')) {
      currentPresetName.value = 'Custom'
    }
  }

  const getDisplayGain = (idx) => {
    const gain = eqGains.value[idx]
    return gain > 0 ? `+${gain}` : `${gain}`
  }

  const formatBandLabel = (band) => {
    if (band >= 1000) {
      return `${band / 1000}k`
    }
    return `${band}`
  }

  const getIconForRoute = (path) => {
    if (path === 'my-music') return 'Song'
    if (path === 'playing') return 'NowPlaying'
    return 'Song'
  }
</script>
