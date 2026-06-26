<template>
  <div
    class="grx-SideNav bg-(--nav-bg) flex flex-col transition-all duration-300 ease-in-out shrink-0"
    :class="isCollapsed ? 'w-14 min-w-14' : 'w-57.5 min-w-57.5'"
    style="backdrop-filter: blur(23px); height: calc(100svh - var(--audio-controller-h))">
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
          <div class="w-[180px] border-r border-theme-border/30 bg-theme-bg-item/40 flex flex-col p-3 gap-1 select-none shrink-0">
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
            <OverlayScrollbarsComponent :options="{ scrollbars: { autoHide: 'scroll' } }" defer class="flex-1 p-4 overflow-y-auto">
              <!-- Cài đặt chung (General) -->
              <div v-if="activeSettingsTab === 'general'" class="flex flex-col gap-4">
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
                  <div
                    class="bg-theme-bg-placeholder/5 border border-theme-border/30 rounded-lg p-3 mb-4 flex items-center justify-between">
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
                  <div
                    class="flex justify-between items-stretch bg-theme-bg-placeholder/5 border border-theme-border/20 rounded-xl h-fit relative">
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

              <!-- Phím tắt (Shortcuts) -->
              <div v-if="activeSettingsTab === 'shortcuts'" class="flex flex-col gap-3 h-full">
                <div class="flex justify-between items-center mb-1">
                  <div class="text-xs text-theme-text-secondary font-medium">Tùy biến phím tắt (JSON giống VSCode)</div>
                  <button
                    @click="resetShortcutsToDefault"
                    class="text-[11px] text-theme-accent-light hover:underline font-semibold cursor-pointer">
                    Đặt lại mặc định
                  </button>
                </div>

                <div class="relative flex-grow flex flex-col min-h-0">
                  <textarea
                    ref="textareaRef"
                    v-model="shortcutsJsonDraft"
                    class="w-full h-[260px] p-3 text-xs font-mono bg-theme-bg-placeholder/20 border border-theme-border rounded-lg focus:border-theme-accent/50 outline-none resize-none transition-colors"
                    placeholder='[\n  { "key": "ctrl+k", "command": "open_search" }\n]'
                    @input="handleTextareaInput"
                    @keydown="handleTextareaKeyDown"
                    @click="checkSuggestions"
                    @blur="onTextareaBlur"></textarea>

                  <!-- Floating Autocomplete Suggestions -->
                  <div
                    v-if="showSuggestions && filteredCommands.length > 0"
                    class="absolute bg-theme-bg-item border border-theme-border rounded-lg shadow-xl z-50 p-1 flex flex-col max-h-[150px] overflow-y-auto w-[180px] font-mono text-[11px]"
                    :style="{ top: suggestionTop + 'px', left: suggestionLeft + 'px' }">
                    <div
                      v-for="(cmd, idx) in filteredCommands"
                      :key="cmd"
                      @click="selectSuggestionIndex(idx)"
                      @mouseenter="activeSuggestionIndex = idx"
                      class="px-2 py-1 rounded cursor-pointer transition-colors"
                      :class="idx === activeSuggestionIndex ? 'bg-theme-accent/15 text-theme-accent-light' : 'text-theme-text-secondary'">
                      {{ cmd }}
                    </div>
                  </div>
                  <div v-if="shortcutsJsonError" class="text-[11px] text-red-400 mt-1 font-semibold flex items-center gap-1.5">
                    <span>⚠️</span>
                    <span>{{ shortcutsJsonError }}</span>
                  </div>
                  <div v-else class="text-[10px] text-theme-text-disabled mt-1 leading-normal">
                    Gợi ý các command:
                    <code class="text-theme-accent-light">open_search</code>
                    ,
                    <code class="text-theme-accent-light">toggle_play</code>
                    ,
                    <code class="text-theme-accent-light">prev_track</code>
                    ,
                    <code class="text-theme-accent-light">next_track</code>
                    ,
                    <code class="text-theme-accent-light">volume_up</code>
                    ,
                    <code class="text-theme-accent-light">volume_down</code>
                    ,
                    <code class="text-theme-accent-light">seek_backward</code>
                    ,
                    <code class="text-theme-accent-light">seek_forward</code>
                    ,
                    <code class="text-theme-accent-light">play_random</code>
                    ,
                    <code class="text-theme-accent-light">go_to_library</code>
                    ,
                    <code class="text-theme-accent-light">go_to_now_playing</code>
                  </div>
                </div>
              </div>
            </OverlayScrollbarsComponent>
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
  import { ref, watch, computed, nextTick } from 'vue';
  import { storeToRefs } from 'pinia';
  import { useAudioPlayer } from '@groovex/store';
  import { OCTAVE_BANDS } from '@groovex/core';
  import appInfo from '../../../app.json' with { type: 'json' };
  import { APP_ROUTES } from '../../app.route.ts';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import CustomBtn from '@groovex/ui/button/custom-btn.vue';
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue';
  import { DEFAULT_KEYBINDINGS } from '../../shared/composables/global-event-listener-top';

  const showSettings = ref(false);
  const isCollapsed = ref(typeof localStorage !== 'undefined' ? localStorage.getItem('side-nav-collapsed') === 'true' : false);
  const player = useAudioPlayer();
  const seekStep = storeToRefs(player).seekStep;
  const volumeStep = storeToRefs(player).volumeStep;
  const eqGains = storeToRefs(player).eqGains;
  const bassBoost = storeToRefs(player).bassBoost;
  const currentPresetName = storeToRefs(player).currentPresetName;

  const textareaRef = ref(null);
  const showSuggestions = ref(false);
  const filteredCommands = ref([]);
  const activeSuggestionIndex = ref(0);
  const suggestionTop = ref(0);
  const suggestionLeft = ref(0);

  const COMMANDS_LIST = [
    'open_search',
    'toggle_play',
    'prev_track',
    'next_track',
    'volume_up',
    'volume_down',
    'seek_backward',
    'seek_forward',
    'play_random',
    'go_to_library',
    'go_to_now_playing',
  ];

  const handleTextareaInput = () => {
    validateShortcutsJson();
    checkSuggestions();
  };

  const checkSuggestions = () => {
    const textarea = textareaRef.value;
    if (!textarea) return;

    const start = textarea.selectionStart;
    const text = shortcutsJsonDraft.value;
    const textBeforeCursor = text.substring(0, start);

    const match = textBeforeCursor.match(/"command"\s*:\s*"([a-zA-Z_]*)$/);
    if (match) {
      const typedText = match[1];
      filteredCommands.value = COMMANDS_LIST.filter((cmd) => cmd.startsWith(typedText));
      if (filteredCommands.value.length > 0) {
        showSuggestions.value = true;
        if (activeSuggestionIndex.value >= filteredCommands.value.length) {
          activeSuggestionIndex.value = 0;
        }
        // Approximate caret position in a monospace font
        const lines = textBeforeCursor.split('\n');
        const currentLineIdx = lines.length - 1;
        const currentLineText = lines[currentLineIdx];
        const charOffset = currentLineText.length;

        suggestionTop.value = Math.min(200, 12 + (currentLineIdx + 1) * 16.5);
        suggestionLeft.value = Math.min(400, 12 + charOffset * 7.1);
        return;
      }
    }
    showSuggestions.value = false;
  };

  const onTextareaBlur = () => {
    setTimeout(() => {
      showSuggestions.value = false;
    }, 150);
  };

  const selectSuggestionIndex = (idx) => {
    activeSuggestionIndex.value = idx;
    insertSelectedSuggestion();
  };

  const insertSelectedSuggestion = () => {
    const textarea = textareaRef.value;
    if (!textarea) return;

    const start = textarea.selectionStart;
    const text = shortcutsJsonDraft.value;
    const textBeforeCursor = text.substring(0, start);
    const match = textBeforeCursor.match(/"command"\s*:\s*"([a-zA-Z_]*)$/);
    if (!match) return;

    const typedLength = match[1].length;
    const suggestion = filteredCommands.value[activeSuggestionIndex.value];

    const textAfterCursor = text.substring(start);
    let insertText = suggestion;
    if (!textAfterCursor.startsWith('"')) {
      insertText = suggestion + '"';
    }

    shortcutsJsonDraft.value = text.substring(0, start - typedLength) + insertText + text.substring(start);

    showSuggestions.value = false;
    nextTick(() => {
      textarea.selectionStart = textarea.selectionEnd = start - typedLength + insertText.length;
      textarea.focus();
      validateShortcutsJson();
    });
  };

  const handleTextareaKeyDown = (e) => {
    const textarea = textareaRef.value;
    if (!textarea) return;

    const start = textarea.selectionStart;
    const end = textarea.selectionEnd;
    const text = shortcutsJsonDraft.value;

    // Handle suggestion menu keys
    if (showSuggestions.value && filteredCommands.value.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault();
        activeSuggestionIndex.value = (activeSuggestionIndex.value + 1) % filteredCommands.value.length;
        return;
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault();
        activeSuggestionIndex.value = (activeSuggestionIndex.value - 1 + filteredCommands.value.length) % filteredCommands.value.length;
        return;
      }
      if (e.key === 'Enter' || e.key === 'Tab') {
        e.preventDefault();
        insertSelectedSuggestion();
        return;
      }
      if (e.key === 'Escape') {
        e.preventDefault();
        showSuggestions.value = false;
        return;
      }
    }

    // Auto-close pairs syntax autocomplete
    if (e.key === '{') {
      e.preventDefault();
      shortcutsJsonDraft.value = text.substring(0, start) + '{}' + text.substring(end);
      nextTick(() => {
        textarea.selectionStart = textarea.selectionEnd = start + 1;
      });
    } else if (e.key === '[') {
      e.preventDefault();
      shortcutsJsonDraft.value = text.substring(0, start) + '[]' + text.substring(end);
      nextTick(() => {
        textarea.selectionStart = textarea.selectionEnd = start + 1;
      });
    } else if (e.key === '"') {
      e.preventDefault();
      if (text[start] === '"') {
        textarea.selectionStart = textarea.selectionEnd = start + 1;
      } else {
        shortcutsJsonDraft.value = text.substring(0, start) + '""' + text.substring(end);
        nextTick(() => {
          textarea.selectionStart = textarea.selectionEnd = start + 1;
        });
      }
    } else if (e.key === '}' && text[start] === '}') {
      e.preventDefault();
      textarea.selectionStart = textarea.selectionEnd = start + 1;
    } else if (e.key === ']' && text[start] === ']') {
      e.preventDefault();
      textarea.selectionStart = textarea.selectionEnd = start + 1;
    }
  };

  const settingsTabs = [
    { id: 'general', name: 'Cài đặt chung' },
    { id: 'shortcuts', name: 'Phím tắt' },
  ];
  const activeSettingsTab = ref('general');

  const shortcutsJsonDraft = ref('');
  const shortcutsJsonError = ref('');

  const validateShortcutsJson = () => {
    try {
      if (!shortcutsJsonDraft.value.trim()) {
        shortcutsJsonError.value = 'JSON không được để trống';
        return false;
      }
      const parsed = JSON.parse(shortcutsJsonDraft.value);
      if (!Array.isArray(parsed)) {
        shortcutsJsonError.value = 'JSON phải là một danh sách (Array)';
        return false;
      }
      for (let i = 0; i < parsed.length; i++) {
        const item = parsed[i];
        if (!item || typeof item !== 'object' || !item.key || !item.command) {
          shortcutsJsonError.value = `Mục số ${i + 1} thiếu trường "key" hoặc "command"`;
          return false;
        }
      }
      shortcutsJsonError.value = '';
      return true;
    } catch (e) {
      shortcutsJsonError.value = 'Định dạng JSON không hợp lệ';
      return false;
    }
  };

  const resetShortcutsToDefault = () => {
    shortcutsJsonDraft.value = JSON.stringify(DEFAULT_KEYBINDINGS, null, 2);
    shortcutsJsonError.value = '';
  };

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
      shortcutsJsonError.value = '';
      const saved = localStorage.getItem('custom-keybindings');
      if (saved) {
        shortcutsJsonDraft.value = JSON.stringify(JSON.parse(saved), null, 2);
      } else {
        shortcutsJsonDraft.value = JSON.stringify(DEFAULT_KEYBINDINGS, null, 2);
      }
    }
  });

  const saveSettings = () => {
    if (shortcutsJsonDraft.value) {
      if (!validateShortcutsJson()) {
        activeSettingsTab.value = 'shortcuts';
        return;
      }
      localStorage.setItem('custom-keybindings', JSON.stringify(JSON.parse(shortcutsJsonDraft.value)));
      window.dispatchEvent(new Event('keybindings-updated'));
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
  ];

  const applyPreset = (preset) => {
    currentPresetName.value = preset.name;
    eqGains.value = [...preset.gains];
  };

  const onSliderChange = () => {
    currentPresetName.value = 'Custom';
  };

  const onBassBoostChange = () => {
    if (currentPresetName.value !== 'Custom' && !currentPresetName.value.includes('Bass')) {
      currentPresetName.value = 'Custom';
    }
  };

  const getDisplayGain = (idx) => {
    const gain = eqGains.value[idx];
    return gain > 0 ? `+${gain}` : `${gain}`;
  };

  // Catmull-Rom spline interpolation to SVG Cubic Bezier path
  const catmullRom2bezier = (points) => {
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
  };

  const curvePath = computed(() => {
    // Generate coordinate points for each of the 10 bands
    // Horizontal center for column i in 1000px wide viewBox: x_i = i * 100 + 50
    // Vertical value: eqGains value goes from -10 to +10.
    // Map -10 to 10 dB to vertical viewport range [2, 99.5] where 2 is top (+10dB) and 99.5 is bottom (-10dB)
    const points = eqGains.value.map((gain, idx) => {
      const x = idx * 100 + 50;
      const percentage = (10 - gain) / 20; // 0 at +10dB, 1 at -10dB
      const y = 2 + percentage * 97.5;
      return { x, y };
    });
    return catmullRom2bezier(points);
  });

  const formatBandLabel = (band) => {
    if (band >= 1000) {
      return `${band / 1000}k`;
    }
    return `${band}`;
  };

  const getIconForRoute = (path) => {
    if (path === 'my-music') return 'Song';
    if (path === 'playing') return 'NowPlaying';
    if (path === 'playlists') return 'Album';
    return 'Song';
  };
</script>
