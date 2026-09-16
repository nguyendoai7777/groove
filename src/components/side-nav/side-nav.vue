<template>
  <div
    class="grx-SideNav bg-(--nav-bg) flex flex-col transition-all duration-300 ease-in-out shrink-0"
    :class="isCollapsed ? 'w-14 min-w-14' : 'w-57.5 min-w-57.5'"
    style="height: calc(100svh - var(--audio-controller-h))">
    <div
      class="h-(--nav-head-h) hover:bg-side-nav-header-hover flex items-center select-none font-light text-xs cursor-default shrink-0 transition-all duration-300"
      :class="isCollapsed ? 'justify-center px-0' : 'justify-between px-3'"
      data-tauri-drag-region>
      <span v-if="!isCollapsed" class="truncate font-semibold tracking-wider">{{ appInfo.name }}</span>
      <custom-btn
        icon
        variant="text"
        density="compact"
        class="text-theme-text-muted hover:text-theme-text rounded-none border-none!"
        @click="toggleCollapse"
        :title="isCollapsed ? 'Expand sidebar' : 'Collapse sidebar'">
        <svg-sprite :src="isCollapsed ? 'AngleRight' : 'AngleLeft'" class="w-4 h-4" />
      </custom-btn>
    </div>

    <div class="flex-1 flex flex-col overflow-x-hidden">
      <RouterLink
        class="flex"
        v-for="nav in APP_ROUTES[0].children.filter((c) => c.meta && c.meta.title)"
        :key="nav.path"
        :to="'/' + nav.path"
        v-slot="{ isActive }">
        <div
          class="group border-l-[3px] transition-all duration-200 flex-1"
          :class="
            isActive
              ? 'border-theme-accent-light bg-theme-bg-placeholder/30'
              : 'border-transparent hover:border-theme-border-hover/70 hover:bg-theme-bg-placeholder/10'
          ">
          <v-btn
            class="w-full flex items-center text-sm h-(--nav-link-h) shadow-none bg-transparent rounded-none min-w-0!"
            :class="isCollapsed ? 'justify-center px-0' : 'justify-start px-4 text-left'"
            :title="isCollapsed ? nav.meta['title'] : ''">
            <svg-sprite
              class="w-4 h-4"
              :src="getIconForRoute(nav.path)"
              :class="[
                isCollapsed ? 'mr-0' : 'mr-3',
                isActive ? 'text-theme-accent-light' : 'text-theme-text-muted group-hover:text-theme-text-secondary',
              ]" />
            <span
              v-if="!isCollapsed"
              class="truncate"
              :class="isActive ? 'font-semibold' : 'text-theme-text-muted group-hover:text-theme-text-secondary'">
              {{ nav.meta['title'] }}
            </span>
          </v-btn>
        </div>
      </RouterLink>
    </div>

    <div class="border-t border-theme-border/30 shrink-0">
      <v-btn
        @click="showSettings = true"
        class="w-full flex items-center text-sm h-(--nav-link-h) shadow-none bg-transparent hover:bg-theme-bg-placeholder/10 rounded-none text-theme-text-muted hover:text-theme-text-secondary border-l-[3px] border-transparent hover:border-theme-border-hover/70 transition-all duration-200 min-w-0!"
        :class="isCollapsed ? 'justify-center px-0' : 'justify-start px-4 text-left'"
        :title="isCollapsed ? 'Settings' : ''">
        <svg-sprite src="Settings" class="w-4 h-4" :class="isCollapsed ? 'mr-0' : 'mr-3'" />
        <span v-if="!isCollapsed" class="truncate">Settings</span>
      </v-btn>
    </div>

    <!-- Settings Dialog -->
    <v-dialog v-model="showSettings" max-width="800">
      <v-card
        class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
        <v-card-title class="text-md! font-bold! border-b border-theme-border/80 px-4 py-3">Settings</v-card-title>
        <v-card-text class="pa-0 flex h-[480px] overflow-hidden">
          <!-- Small internal side nav for settings -->
          <div
            class="w-now-playing-left-min-w border-r border-theme-border/30 bg-theme-bg-item/40 flex flex-col p-3 gap-1 select-none shrink-0">
            <button
              v-for="tab in settingsTabs"
              :key="tab.id"
              @click="activeSettingsTab = tab.id"
              class="text-left px-3 py-2 rounded-lg text-xs font-semibold transition-colors duration-150 cursor-pointer"
              :class="
                activeSettingsTab === tab.id
                  ? 'bg-theme-accent/15 text-theme-accent-light'
                  : 'text-theme-text-muted hover:bg-theme-bg-placeholder/45 hover:text-theme-text-secondary'
              ">
              {{ tab.name }}
            </button>
          </div>

          <!-- Right content area -->
          <div class="flex-1 flex flex-col overflow-hidden bg-theme-bg-item">
            <overlay-scrollbars-component :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="flex-1 p-4 overflow-y-auto">
              <!-- Cài đặt chung (General) -->
              <setting-general v-show="activeSettingsTab === 'general'" />

              <!-- Phím tắt (Shortcuts) -->
              <setting-keyboard-binding ref="keyboardBindingRef" v-show="activeSettingsTab === 'shortcuts'" :is-open="showSettings" />
            </overlay-scrollbars-component>
          </div>
        </v-card-text>
        <v-card-actions class="px-4 py-3 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
          <custom-btn variant="secondary" @click="cancelSettings">Cancel</custom-btn>
          <custom-btn variant="primary" @click="saveSettings">OK</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup>
  import { ref, watch } from 'vue';
  import { storeToRefs } from 'pinia';
  import { useAudioPlayer } from '@groovex/store';
  import appInfo from '../../../app.json' with { type: 'json' };
  import { APP_ROUTES } from '../../app.route.ts';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import CustomBtn from '@groovex/ui/button/custom-btn.vue';
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue';
  import SettingGeneral from './setting-general.vue';
  import SettingKeyboardBinding from './setting-keyboard-binding.vue';

  const showSettings = ref(false);
  const isCollapsed = ref(typeof localStorage !== 'undefined' ? localStorage.getItem('side-nav-collapsed') === 'true' : false);
  const player = useAudioPlayer();
  const seekStep = storeToRefs(player).seekStep;
  const volumeStep = storeToRefs(player).volumeStep;
  const eqGains = storeToRefs(player).eqGains;
  const bassBoost = storeToRefs(player).bassBoost;
  const currentPresetName = storeToRefs(player).currentPresetName;

  const keyboardBindingRef = ref();

  const settingsTabs = [
    { id: 'general', name: 'Cài đặt chung' },
    { id: 'shortcuts', name: 'Phím tắt' },
  ];
  const activeSettingsTab = ref('general');

  const toggleCollapse = () => {
    isCollapsed.value = !isCollapsed.value;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('side-nav-collapsed', String(isCollapsed.value));
    }
  };

  // Backup variables for OK/Cancel transactions
  let originalSeekStep = 5;
  let originalVolumeStep = 2;
  let originalEqGains = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  let originalBassBoost = 0;
  let originalPresetName = 'Flat';

  // Backup values when the settings modal opens
  watch(showSettings, (open) => {
    if (open) {
      originalSeekStep = seekStep.value;
      originalVolumeStep = volumeStep.value;
      originalEqGains = [...eqGains.value];
      originalBassBoost = bassBoost.value;
      originalPresetName = currentPresetName.value;

      activeSettingsTab.value = 'general';
    }
  });

  const saveSettings = () => {
    if (keyboardBindingRef.value) {
      const success = keyboardBindingRef.value.save();
      if (!success) {
        activeSettingsTab.value = 'shortcuts';
        return;
      }
    }
    showSettings.value = false;
  };

  const cancelSettings = () => {
    seekStep.value = originalSeekStep;
    volumeStep.value = originalVolumeStep;
    eqGains.value = [...originalEqGains];
    bassBoost.value = originalBassBoost;
    currentPresetName.value = originalPresetName;
    showSettings.value = false;
  };

  const getIconForRoute = (path) => {
    if (path === 'my-music') return 'Song';
    if (path === 'playing') return 'NowPlaying';
    if (path === 'playlists') return 'Album';
    return 'Song';
  };
</script>
