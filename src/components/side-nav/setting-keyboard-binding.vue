<template>
  <div class="grx-SettingKeyboardBinding flex flex-col gap-3 h-full">
    <div class="flex justify-between items-center mb-1">
      <div class="text-xs text-theme-text-secondary font-medium">Tùy biến phím tắt (JSON giống VSCode)</div>
      <button @click="resetShortcutsToDefault" class="text-[11px] text-theme-accent-light hover:underline font-semibold cursor-pointer">
        Đặt lại mặc định
      </button>
    </div>

    <div class="relative grow flex flex-col min-h-0">
      <textarea
        ref="textareaRef"
        v-model="shortcutsJsonDraft"
        spellcheck="false"
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
</template>

<script setup lang="ts">
  import { ref, watch, nextTick } from 'vue';
  import { DEFAULT_KEYBINDINGS } from '../../shared/composables/global-event-listener-top';

  const props = defineProps<{
    isOpen: boolean;
  }>();

  const shortcutsJsonDraft = ref('');
  const shortcutsJsonError = ref('');

  const textareaRef = ref<HTMLTextAreaElement | null>(null);
  const showSuggestions = ref(false);
  const filteredCommands = ref<string[]>([]);
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

  const selectSuggestionIndex = (idx: number) => {
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

  const handleTextareaKeyDown = (e: KeyboardEvent) => {
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

  watch(
    () => props.isOpen,
    (open) => {
      if (open) {
        shortcutsJsonError.value = '';
        const saved = localStorage.getItem('custom-keybindings');
        if (saved) {
          try {
            shortcutsJsonDraft.value = JSON.stringify(JSON.parse(saved), null, 2);
          } catch {
            shortcutsJsonDraft.value = JSON.stringify(DEFAULT_KEYBINDINGS, null, 2);
          }
        } else {
          shortcutsJsonDraft.value = JSON.stringify(DEFAULT_KEYBINDINGS, null, 2);
        }
      }
    },
    { immediate: true },
  );

  function save(): boolean {
    if (shortcutsJsonDraft.value) {
      if (!validateShortcutsJson()) {
        return false;
      }
      localStorage.setItem('custom-keybindings', JSON.stringify(JSON.parse(shortcutsJsonDraft.value)));
      window.dispatchEvent(new Event('keybindings-updated'));
    }
    return true;
  }

  defineExpose({
    save,
  });
</script>
