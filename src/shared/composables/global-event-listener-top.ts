// composables/useKeyboardShortcuts.ts
import { onMounted, onUnmounted, watch, ref } from 'vue';
import { useRouter } from 'vue-router';
import { useCommandPaletteStore, useAudioPlayer } from '@groovex/store';

export interface Keybinding {
  key: string;
  command: string;
}

export const DEFAULT_KEYBINDINGS: Keybinding[] = [
  { key: 'ctrl+k', command: 'open_search' },
  { key: 'space', command: 'toggle_play' },
  { key: 'arrowleft', command: 'seek_backward' },
  { key: 'arrowright', command: 'seek_forward' },
  { key: 'alt+arrowleft', command: 'prev_track' },
  { key: 'alt+arrowright', command: 'next_track' },
  { key: 'arrowup', command: 'volume_up' },
  { key: 'arrowdown', command: 'volume_down' },
  { key: 'alt+p', command: 'play_random' },
  { key: 'g m', command: 'go_to_library' },
  { key: 'g p', command: 'go_to_now_playing' },
];

interface KeyboardCommand {
  key: string;
  ctrl?: boolean;
  shift?: boolean;
  alt?: boolean;
  action(): void;
}

export function useKeyboardShortcuts() {
  const commandPalette = useCommandPaletteStore();
  const player = useAudioPlayer();
  const router = useRouter();

  // Load custom keybindings from localStorage with fallback to default
  const loadKeybindings = (): Keybinding[] => {
    try {
      const saved = localStorage.getItem('custom-keybindings');
      if (saved) {
        return JSON.parse(saved);
      }
    } catch (e) {
      console.error('Failed to parse custom keybindings, using defaults', e);
    }
    return DEFAULT_KEYBINDINGS;
  };

  const keybindings = ref<Keybinding[]>(loadKeybindings());

  const reloadKeybindings = () => {
    keybindings.value = loadKeybindings();
  };

  // ----------------------------------------------------
  // 1. CÁC PHÍM TẮT CẤP 2 (Chỉ chạy khi Dialog ĐANG MỞ)
  // ----------------------------------------------------
  const dialogShortcuts: KeyboardCommand[] = [
    {
      key: 'd',
      ctrl: true,
      action: () => console.log('Chuyển chế độ Dark mode!'),
    },
    {
      key: 'p',
      ctrl: true,
      action: () => console.log('Đi tới trang cá nhân!'),
    },
  ];

  const handleDialogKeyDown = (event: KeyboardEvent) => {
    const pressedKey = event.key.toLowerCase();
    const isCtrlPressed = event.ctrlKey || event.metaKey;
    const isShiftPressed = event.shiftKey;
    const isAltPressed = event.altKey;

    const matched = dialogShortcuts.find((s) => {
      return s.key.toLowerCase() === pressedKey && !!s.ctrl === isCtrlPressed && !!s.shift === isShiftPressed && !!s.alt === isAltPressed;
    });

    if (matched) {
      event.preventDefault();
      matched.action();
    }
  };

  // ----------------------------------------------------
  // 2. PHÍM TẮT CẤP 1 (Luôn lắng nghe toàn cục)
  // ----------------------------------------------------
  const getKeyEventString = (e: KeyboardEvent): string => {
    const parts: string[] = [];
    if (e.ctrlKey || e.metaKey) parts.push('ctrl');
    if (e.altKey) parts.push('alt');
    if (e.shiftKey) parts.push('shift');

    const key = e.key.toLowerCase();
    if (key !== 'control' && key !== 'alt' && key !== 'shift' && key !== 'meta') {
      if (key === ' ' || key === 'spacebar') {
        parts.push('space');
      } else {
        parts.push(key);
      }
    }
    return parts.join('+');
  };

  const pressedHistory = ref<string[]>([]);
  let historyTimeout: number | undefined;

  const executeCommand = (command: string) => {
    switch (command) {
      case 'open_search':
        commandPalette.open();
        break;
      case 'toggle_play':
        if (player.currentSong) {
          player.togglePlay();
        }
        break;
      case 'prev_track':
        if (player.currentSong) {
          player.prevTrack();
        }
        break;
      case 'next_track':
        if (player.currentSong) {
          player.nextTrack();
        }
        break;
      case 'volume_up': {
        const step = Number(player.volumeStep);
        const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 2 : step;
        player.volume = Math.min(100, (player.volume || 0) + finalStep);
        break;
      }
      case 'volume_down': {
        const step = Number(player.volumeStep);
        const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 2 : step;
        player.volume = Math.max(0, (player.volume || 0) - finalStep);
        break;
      }
      case 'seek_backward': {
        if (player.currentSong) {
          const step = Number(player.seekStep);
          const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 5 : step;
          player.seek(Math.max(0, (player.currentTime || 0) - finalStep));
        }
        break;
      }
      case 'seek_forward': {
        if (player.currentSong) {
          const step = Number(player.seekStep);
          const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 5 : step;
          player.seek(Math.min(player.duration || 0, (player.currentTime || 0) + finalStep));
        }
        break;
      }
      case 'play_random':
        if (player.playlist && player.playlist.length > 0) {
          player.isShuffle = true;
          player.playSong(player.playlist[Math.floor(Math.random() * player.playlist.length)]);
        }
        break;
      case 'go_to_library':
        router.push('/my-music');
        break;
      case 'go_to_now_playing':
        router.push('/playing');
        break;
    }
  };

  const handleGlobalKeyDown = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement;
    // Don't trigger keybinds when typing in input, textarea, or contenteditable
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      if (event.key.toLowerCase() === 'escape' && commandPalette.isOpen) {
        commandPalette.close();
      }
      return;
    }

    const currentKey = getKeyEventString(event);
    if (!currentKey) return;

    // Track sequence history (max 2 keys)
    pressedHistory.value.push(currentKey);
    if (pressedHistory.value.length > 2) {
      pressedHistory.value.shift();
    }

    // Reset sequence history after a timeout of 800ms
    if (historyTimeout) window.clearTimeout(historyTimeout);
    historyTimeout = window.setTimeout(() => {
      pressedHistory.value = [];
    }, 800);

    const fullSequence = pressedHistory.value.join(' ');

    // 1. Match full sequence (e.g. "g m")
    let matched = keybindings.value.find((k) => k.key.toLowerCase().trim() === fullSequence);
    if (matched) {
      event.preventDefault();
      executeCommand(matched.command);
      pressedHistory.value = [];
      return;
    }

    // 2. Match single combo (e.g. "ctrl+k")
    matched = keybindings.value.find((k) => k.key.toLowerCase().trim() === currentKey);
    if (matched) {
      event.preventDefault();
      executeCommand(matched.command);
      pressedHistory.value = [];
      return;
    }
  };

  // ----------------------------------------------------
  // 3. QUẢN LÝ VÒNG ĐỜI (Lifecycle & Watcher)
  // ----------------------------------------------------
  watch(
    () => commandPalette.isOpen,
    (isOpenNow) => {
      if (isOpenNow) {
        window.addEventListener('keydown', handleDialogKeyDown);
      } else {
        window.removeEventListener('keydown', handleDialogKeyDown);
      }
    },
  );

  const initShortcuts = () => {
    onMounted(() => {
      window.addEventListener('keydown', handleGlobalKeyDown);
      window.addEventListener('keybindings-updated', reloadKeybindings);
    });

    onUnmounted(() => {
      window.removeEventListener('keydown', handleGlobalKeyDown);
      window.removeEventListener('keybindings-updated', reloadKeybindings);
      window.removeEventListener('keydown', handleDialogKeyDown);
    });
  };

  return {
    initShortcuts,
  };
}
