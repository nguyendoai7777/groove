<template>
  <div
    class="min-w-57.5 bg-(--nav-bg) flex flex-col"
    style="backdrop-filter: blur(23px); height: calc(100svh - var(--audio-controller-h))"
  >
    <div
      class="h-(--nav-head-h) duration-200 hover:bg-[#242424FF] flex items-center px-2 select-none font-light text-xs cursor-default shrink-0"
      data-tauri-drag-region
    >
      {{ appInfo.name }}
    </div>

    <div class="flex-1 flex flex-col overflow-y-auto">
      <RouterLink
        v-for="nav in APP_ROUTES[0].children.filter((c) => c.meta && c.meta.title)"
        :key="nav.path"
        :to="'/' + nav.path"
        v-slot="{ isActive }"
      >
        <div
          class="group border-l-[3px] transition-all duration-200"
          :class="
            isActive
              ? 'border-cyan-400 bg-zinc-800/30'
              : 'border-transparent hover:border-zinc-800/70 hover:bg-zinc-800/10'
          "
        >
          <v-btn
            class="w-full flex justify-start items-center px-4 text-sm h-(--nav-link-h) shadow-none bg-transparent rounded-none text-left"
          >
            <svg-sprite
              :src="getIconForRoute(nav.path)"
              class="w-4 h-4 mr-3"
              :class="isActive ? 'text-cyan-400' : 'text-zinc-400 group-hover:text-zinc-200'"
            />
            <span
              :class="
                isActive ? 'text-white font-semibold' : 'text-zinc-400 group-hover:text-zinc-200'
              "
              >{{ nav.meta['title'] }}</span
            >
          </v-btn>
        </div>
      </RouterLink>
    </div>

    <div class="border-t border-zinc-800/30 shrink-0">
      <v-btn
        @click="showSettings = true"
        class="w-full flex justify-start items-center px-4 text-sm h-(--nav-link-h) shadow-none bg-transparent hover:bg-zinc-800/10 rounded-none text-left text-zinc-400 hover:text-zinc-200 border-l-[3px] border-transparent hover:border-zinc-800/70 transition-all duration-200"
      >
        <svg-sprite src="Settings" class="w-4 h-4 mr-3" />
        <span>Settings</span>
      </v-btn>
    </div>

    <!-- Settings Dialog -->
    <v-dialog v-model="showSettings" max-width="450">
      <v-card
        class="grx-ConfirmerCard bg-zinc-900! text-zinc-100! border border-zinc-800! rounded-xl! overflow-hidden shadow-2xl"
      >
        <v-card-title class="text-md! font-bold! border-b border-zinc-800/80 px-6 py-4 text-white">
          Settings
        </v-card-title>

        <v-card-text class="px-6! py-5! text-sm! text-zinc-400">
          GrooveX Application Preferences. You can configure audio quality, hotkeys, theme
          preferences, and sync folders here.
        </v-card-text>

        <v-card-actions
          class="px-6 py-4 flex justify-end gap-2 bg-zinc-950/20 border-t border-zinc-800/50"
        >
          <button
            class="px-4 py-2 text-xs font-semibold rounded-md border border-zinc-800 hover:border-zinc-700 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-all cursor-pointer"
            @click="showSettings = false"
          >
            Cancel
          </button>
          <button
            class="px-4 py-2 text-xs font-semibold rounded-md bg-cyan-600 hover:bg-cyan-500 text-white transition-all shadow-md cursor-pointer"
            @click="showSettings = false"
          >
            OK
          </button>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup>
  import { ref } from 'vue'
  import appInfo from '../../../app.json' with { type: 'json' }
  import { APP_ROUTES } from '../../app.route.ts'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'

  const showSettings = ref(false)

  const getIconForRoute = (path) => {
    if (path === 'my-music') return 'Song'
    if (path === 'playing') return 'NowPlaying'
    return 'Song'
  }
</script>
