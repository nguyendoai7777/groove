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
        <v-card-text class="px-6! py-5! text-sm! text-theme-text-muted flex flex-col gap-4">
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
        </v-card-text>

        <v-card-actions class="px-6 py-4 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
          <custom-btn variant="secondary" @click="showSettings = false">Cancel</custom-btn>
          <custom-btn variant="primary" @click="showSettings = false">OK</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup>
  import { ref } from 'vue'
  import { storeToRefs } from 'pinia'
  import { useAudioPlayer } from '@groovex/state'
  import appInfo from '../../../app.json' with { type: 'json' }
  import { APP_ROUTES } from '../../app.route.ts'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'

  const showSettings = ref(false)
  const player = useAudioPlayer()
  const { seekStep, volumeStep } = storeToRefs(player)

  const getIconForRoute = (path) => {
    if (path === 'my-music') return 'Song'
    if (path === 'playing') return 'NowPlaying'
    return 'Song'
  }
</script>
