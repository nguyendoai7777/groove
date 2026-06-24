import { ref, watch, computed, nextTick } from 'vue';
import { defineStore } from 'pinia';
import { convertFileSrc, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import {
  initialize as initTaskbar,
  isSupported as isTaskbarSupported,
  setNavigationEnabled as setTaskbarNavigation,
  setPlaybackState as setTaskbarPlaybackState,
} from 'tauri-plugin-taskbar';
import type { Song } from '@groovex/types';
import { EStoreKey } from './stores.definition';
import { AudioEngine, StorageLocal, StorageKey, StorageKeys } from '@groovex/core';

export interface PlaybackSong extends Song {
  thumbnail?: string;
}

export const useAudioPlayer = defineStore(EStoreKey.Player, () => {
  // ==========================================
  // 1. STATE / VARIABLES / CONSTANTS
  // ==========================================
  const currentSong = ref<PlaybackSong | null>(getStorageItem(StorageKeys.currentSong, null));
  const isPlaying = ref(false);
  const currentTime = ref(getStorageItem(StorageKeys.currentTime, 0, Number));
  const duration = ref(currentSong.value?.duration || 0);
  const volume = ref(getStorageItem(StorageKeys.volume, 75, Number));
  const isMuted = ref(getStorageItem(StorageKeys.isMuted, false, (v) => v === 'true'));
  const isShuffle = ref(getStorageItem(StorageKeys.isShuffle, false, (v) => v === 'true'));
  const isRepeat = ref(getStorageItem(StorageKeys.isRepeat, false, (v) => v === 'true'));
  const seekStep = ref(
    getStorageItem(StorageKeys.seekStep, 5, (v) => {
      const n = Number(v);
      return n > 0 ? n : 5;
    }),
  );
  const volumeStep = ref(
    getStorageItem(StorageKeys.volumeStep, 2, (v) => {
      const n = Number(v);
      return n > 0 ? n : 2;
    }),
  );
  const bassBoost = ref<number>(getStorageItem(StorageKeys.bassBoost, 0, Number));
  const currentPresetName = ref<string>(getStorageItem(StorageKeys.currentPresetName, 'Flat'));
  const eqGains = ref<number[]>(
    getStorageItem(StorageKeys.eqGains, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0], (v) => {
      const arr = JSON.parse(v);
      return Array.isArray(arr) && arr.length === 10 ? arr.map(Number) : [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    }),
  );

  const originalPlaylist = ref<PlaybackSong[]>(getStorageItem(StorageKeys.originalPlaylist, []));
  const historyPlaylist = ref<PlaybackSong[]>(getInitialHistoryPlaylist());
  const queuePlaylist = ref<PlaybackSong[]>(getInitialQueuePlaylist());

  const playlist = computed<PlaybackSong[]>(() => {
    const list: PlaybackSong[] = [...historyPlaylist.value];
    if (currentSong.value) list.push(currentSong.value);
    list.push(...queuePlaylist.value);
    return list;
  });

  const currentIndex = computed<number>(() => {
    if (!currentSong.value) return -1;
    return historyPlaylist.value.length;
  });

  // --- AUDIO ENGINE ---
  const audio = new Audio();
  audio.crossOrigin = 'anonymous';
  const audioEngine = new AudioEngine(audio);
  const BASS_BOOST_OFFSETS = [1.0, 0.8, 0.5, 0.2];
  const effectiveGains = computed(() => {
    return eqGains.value.map((gain, idx) => {
      const offset = (BASS_BOOST_OFFSETS[idx] ?? 0) * bassBoost.value;
      return Math.max(-10, Math.min(10, gain + offset));
    });
  });

  // --- PLAYBACK HELPERS STATE ---
  let isSeeking = false;
  let isInitialLoad = !!currentSong.value;
  let targetSeekTime: number | null = null;
  let lastSavedTime = currentTime.value;
  let lastSongId = currentSong.value?.id || null;
  let taskbarSetupDone = false;

  // ==========================================
  // 2. FUNCTIONS / HELPERS / ACTIONS
  // ==========================================

  // --- HELPERS FOR STORAGE ---
  function getStorageItem<T>(key: StorageKey, defaultValue: T, parser?: (val: string) => T): T {
    const val = StorageLocal.getItem<any>(key);
    if (val === null || val === '') return defaultValue;
    if (parser && typeof val === 'string') {
      const parsed = parser(val);
      return typeof parsed === 'number' && isNaN(parsed) ? defaultValue : parsed;
    }
    return val as T;
  }

  function setStorage(key: StorageKey, val: any) {
    if (val === null || val === undefined) {
      StorageLocal.removeItem(key);
    } else {
      StorageLocal.setItem(key, val);
    }
  }

  function getInitialHistoryPlaylist() {
    const val = getStorageItem<PlaybackSong[] | null>(StorageKeys.historyPlaylist, null);
    if (val) return val;
    const playlistLegacy = getStorageItem<PlaybackSong[]>(StorageKeys.playlist, []);
    const currentIndexLegacy = Number(getStorageItem(StorageKeys.currentIndex, -1));
    return playlistLegacy.length > 0 && currentIndexLegacy > 0 ? playlistLegacy.slice(0, currentIndexLegacy) : [];
  }

  function getInitialQueuePlaylist() {
    const val = getStorageItem<PlaybackSong[] | null>(StorageKeys.queuePlaylist, null);
    if (val) return val;
    const playlistLegacy = getStorageItem<PlaybackSong[]>(StorageKeys.playlist, []);
    const currentIndexLegacy = Number(getStorageItem(StorageKeys.currentIndex, -1));
    return playlistLegacy.length > 0 && currentIndexLegacy !== -1 ? playlistLegacy.slice(currentIndexLegacy + 1) : [];
  }

  // --- PLAYLIST HELPERS ---
  function cleanPlaylist(list: PlaybackSong[]) {
    return list.map(({ thumbnail, ...rest }) => rest);
  }

  function watchAndSavePlaylist(target: any, key: StorageKey) {
    watch(target, (newVal) => setStorage(key, cleanPlaylist(newVal)), { deep: true });
  }

  // --- PLAYBACK CONTROL HELPERS ---
  async function startPlayback() {
    try {
      audioEngine.resume();
      await audio.play();
      isPlaying.value = true;
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        console.error('Playback failed:', err);
      }
      isPlaying.value = false;
    }
  }

  function pausePlayback() {
    audio.pause();
    isPlaying.value = false;
    setStorage(StorageKeys.currentTime, audio.currentTime);
    lastSavedTime = audio.currentTime;
  }

  function updateCurrentTime() {
    if (targetSeekTime !== null) {
      const diff = audio.currentTime - targetSeekTime;
      if (diff >= 0 || diff < -1.5) {
        currentTime.value = audio.currentTime;
        targetSeekTime = null;
      } else {
        currentTime.value = targetSeekTime;
      }
    } else {
      currentTime.value = audio.currentTime;
    }
  }

  // --- ACTIONS ---
  async function loadAndPlay(song: PlaybackSong) {
    currentSong.value = song;
    currentTime.value = 0;
    duration.value = song.duration;
    setStorage(StorageKeys.currentTime, 0);
    lastSavedTime = 0;

    invoke<any>('get_song_metadata', { songId: song.id })
      .then((meta) => {
        if (meta?.thumbnail && currentSong.value?.id === song.id) {
          currentSong.value.thumbnail = meta.thumbnail;
        }
      })
      .catch((err) => console.error('Failed to get song metadata during loadAndPlay:', err));

    try {
      audio.src = convertFileSrc(song.file_path);
      audio.load();
      await startPlayback();
    } catch (err: any) {
      if (err.name !== 'AbortError') console.error('Playback error:', err);
      isPlaying.value = false;
    }
  }

  async function playSong(song: PlaybackSong, newPlaylist?: PlaybackSong[]) {
    if (currentSong.value?.id === song.id) {
      if (!isPlaying.value) await startPlayback();
      return;
    }

    console.log(
      '[Player] playSong. song =',
      song.title || song.filename,
      'hasNewPlaylist =',
      !!newPlaylist,
      'isShuffle =',
      isShuffle.value,
    );

    if (newPlaylist?.length) {
      originalPlaylist.value = newPlaylist;
      const idx = newPlaylist.findIndex((s) => s.id === song.id);
      historyPlaylist.value = [];
      if (idx !== -1 && !isShuffle.value) {
        queuePlaylist.value = newPlaylist.slice(idx + 1);
      } else {
        queuePlaylist.value = newPlaylist.filter((s) => s.id !== song.id);
      }
    } else {
      const oldSong = currentSong.value;
      if (oldSong && oldSong.id !== song.id) {
        const alreadyInHistory = historyPlaylist.value.some((s) => s.id === oldSong.id);
        if (!alreadyInHistory) {
          historyPlaylist.value.push(oldSong);
        }
      }
      historyPlaylist.value = historyPlaylist.value.filter((s) => s.id !== song.id);
      queuePlaylist.value = queuePlaylist.value.filter((s) => s.id !== song.id);
    }

    await loadAndPlay(song);
  }

  function togglePlay() {
    if (!currentSong.value) return;
    if (isPlaying.value) pausePlayback();
    else startPlayback();
  }

  function setPlaying(state: boolean) {
    if (!currentSong.value) return;
    if (state) startPlayback();
    else pausePlayback();
  }

  function seek(seconds: number) {
    if (!currentSong.value) return;
    isSeeking = true;
    targetSeekTime = seconds;
    audio.currentTime = seconds;
    currentTime.value = seconds;
    setStorage(StorageKeys.currentTime, seconds);
    lastSavedTime = seconds;
  }

  function nextTrack() {
    console.log('[Player] nextTrack called. Queue length:', queuePlaylist.value.length, 'isShuffle =', isShuffle.value);
    if (originalPlaylist.value.length === 0) return;

    const oldSong = currentSong.value;
    let nextSong: PlaybackSong | null = null;

    if (queuePlaylist.value.length > 0) {
      if (isShuffle.value) {
        const randIdx = Math.floor(Math.random() * queuePlaylist.value.length);
        nextSong = queuePlaylist.value[randIdx];
        queuePlaylist.value.splice(randIdx, 1);
      } else {
        nextSong = queuePlaylist.value[0];
        queuePlaylist.value.shift();
      }

      if (oldSong) {
        historyPlaylist.value.push(oldSong);
      }
      loadAndPlay(nextSong);
    } else {
      if (isRepeat.value) {
        historyPlaylist.value = [];
        const candidates = originalPlaylist.value.filter((s) => s.id !== oldSong?.id);
        if (candidates.length > 0) {
          if (isShuffle.value) {
            const randIdx = Math.floor(Math.random() * candidates.length);
            nextSong = candidates[randIdx];
            queuePlaylist.value = candidates.filter((_, i) => i !== randIdx);
          } else {
            nextSong = originalPlaylist.value[0];
            queuePlaylist.value = originalPlaylist.value.slice(1);
          }
        } else {
          nextSong = oldSong;
          queuePlaylist.value = [];
        }
        if (nextSong) {
          loadAndPlay(nextSong);
        }
      } else {
        isPlaying.value = false;
        audio.currentTime = 0;
        setStorage(StorageKeys.currentTime, 0);
        lastSavedTime = 0;
      }
    }
  }

  function prevTrack() {
    if (historyPlaylist.value.length === 0) {
      seek(0);
      return;
    }
    const prevSong = historyPlaylist.value[historyPlaylist.value.length - 1];
    const oldSong = currentSong.value;

    if (prevSong) {
      console.log('[Player] playing prev song:', prevSong.title || prevSong.filename);
      historyPlaylist.value.pop();
      if (oldSong) {
        queuePlaylist.value.unshift(oldSong);
      }
      loadAndPlay(prevSong);
    }
  }

  function toggleShuffle() {
    isShuffle.value = !isShuffle.value;
  }

  function toggleRepeat() {
    isRepeat.value = !isRepeat.value;
  }

  // --- HELPER FOR BULK UPDATES ---
  function updateSongProperty(songId: number, callback: (song: PlaybackSong) => void) {
    if (currentSong.value && currentSong.value.id === songId) {
      callback(currentSong.value);
    }
    const hSong = historyPlaylist.value.find((s) => s.id === songId);
    if (hSong) callback(hSong);
    const qSong = queuePlaylist.value.find((s) => s.id === songId);
    if (qSong) callback(qSong);
  }

  function updateLyrics(songId: number, lyrics: string) {
    updateSongProperty(songId, (s) => {
      s.lyrics = lyrics;
    });
  }

  function updateTimeline(songId: number, timeline: string) {
    updateSongProperty(songId, (s) => {
      s.timeline = timeline;
    });
  }

  async function updateSongMetadata(
    songId: number,
    metadata: {
      filename: string;
      title: string;
      artist: string;
      album_name: string;
      track_number: string;
      thumbnail: string;
    },
  ) {
    try {
      const updatedSong = await invoke<Song>('update_song_metadata', {
        songId,
        filename: metadata.filename,
        title: metadata.title,
        artist: metadata.artist,
        albumName: metadata.album_name,
        trackNumber: metadata.track_number,
        newThumbnail: metadata.thumbnail,
      });

      updateSongProperty(songId, (s) => {
        s.title = updatedSong.title;
        s.artist = updatedSong.artist;
        s.album_id = updatedSong.album_id;
        s.file_path = updatedSong.file_path;
        s.filename = updatedSong.filename;
        if (metadata.thumbnail) s.thumbnail = metadata.thumbnail;
      });

      return updatedSong;
    } catch (err) {
      console.error('Failed to update song metadata:', err);
      throw err;
    }
  }

  // --- TASKBAR MEDIA CONTROLS SETUP ---
  async function initTaskbarControls() {
    try {
      if (await isTaskbarSupported()) {
        await initTaskbar();
        taskbarSetupDone = true;

        await setTaskbarPlaybackState(isPlaying.value);
        await setTaskbarNavigation(!!currentSong.value, !!currentSong.value);

        await listen('media-prev', prevTrack);
        await listen('media-toggle', togglePlay);
        await listen('media-next', nextTrack);
      }
    } catch (err) {
      console.error('Failed to initialize taskbar controls:', err);
    }
  }

  const updateTaskbarState = async () => {
    if (taskbarSetupDone) {
      try {
        await setTaskbarPlaybackState(isPlaying.value);
        await setTaskbarNavigation(!!currentSong.value, !!currentSong.value);
      } catch (err) {
        console.error('Failed to update taskbar state:', err);
      }
    }
  };

  // ==========================================
  // 3. WATCHERS / EVENT LISTENERS / CALLING CODE
  // ==========================================

  // --- PLAYLIST WATCHERS ---
  watchAndSavePlaylist(originalPlaylist, StorageKeys.originalPlaylist);
  watchAndSavePlaylist(historyPlaylist, StorageKeys.historyPlaylist);
  watchAndSavePlaylist(queuePlaylist, StorageKeys.queuePlaylist);
  watchAndSavePlaylist(playlist, StorageKeys.playlist);

  // --- STATE WATCHERS ---
  watch(seekStep, (newVal) => setStorage(StorageKeys.seekStep, newVal));
  watch(volumeStep, (newVal) => setStorage(StorageKeys.volumeStep, newVal));
  watch(currentIndex, (newVal) => setStorage(StorageKeys.currentIndex, newVal));
  watch(bassBoost, (newVal) => setStorage(StorageKeys.bassBoost, newVal));
  watch(currentPresetName, (newVal) => setStorage(StorageKeys.currentPresetName, newVal));
  watch(eqGains, (newVal) => setStorage(StorageKeys.eqGains, newVal), { deep: true });

  watch(isShuffle, (newVal) => {
    console.log('[Player] isShuffle watch triggered. newVal =', newVal);
    setStorage(StorageKeys.isShuffle, newVal);
    if (newVal) {
      isRepeat.value = false;
      if (currentSong.value) {
        const playedIds = new Set(historyPlaylist.value.map((s) => s.id));
        queuePlaylist.value = originalPlaylist.value.filter((s) => s.id !== currentSong.value!.id && !playedIds.has(s.id));
      }
    } else {
      if (currentSong.value) {
        const idx = originalPlaylist.value.findIndex((s) => s.id === currentSong.value!.id);
        if (idx !== -1) {
          queuePlaylist.value = originalPlaylist.value.slice(idx + 1);
        }
      }
    }
  });

  function enforceUniqueness() {
    const currentId = currentSong.value?.id;

    const uniqueHistory: PlaybackSong[] = [];
    const historyIds = new Set<number>();
    historyPlaylist.value.forEach((song) => {
      if (song.id !== currentId && !historyIds.has(song.id)) {
        uniqueHistory.push(song);
        historyIds.add(song.id);
      }
    });

    if (JSON.stringify(historyPlaylist.value.map((s) => s.id)) !== JSON.stringify(uniqueHistory.map((s) => s.id))) {
      historyPlaylist.value = uniqueHistory;
    }

    const uniqueQueue: PlaybackSong[] = [];
    const queueIds = new Set<number>();
    queuePlaylist.value.forEach((song) => {
      if (song.id !== currentId && !historyIds.has(song.id) && !queueIds.has(song.id)) {
        uniqueQueue.push(song);
        queueIds.add(song.id);
      }
    });

    if (JSON.stringify(queuePlaylist.value.map((s) => s.id)) !== JSON.stringify(uniqueQueue.map((s) => s.id))) {
      queuePlaylist.value = uniqueQueue;
    }
  }

  watch(
    [currentSong, historyPlaylist, queuePlaylist],
    () => {
      enforceUniqueness();
    },
    { deep: true, immediate: true },
  );

  watch(isRepeat, (newVal) => {
    setStorage(StorageKeys.isRepeat, newVal);
    if (newVal && isShuffle.value) isShuffle.value = false;
  });

  watch(
    currentSong,
    (newVal) => {
      if (!newVal || newVal.id !== lastSongId) {
        targetSeekTime = null;
        isSeeking = false;
        lastSongId = newVal?.id || null;
      }
      setStorage(StorageKeys.currentSong, newVal);
    },
    { deep: true },
  );

  watch(volume, (newVal) => {
    audioEngine.setVolume(newVal / 100);
    setStorage(StorageKeys.volume, newVal);
  });

  watch(isMuted, (newVal) => {
    audio.muted = newVal;
    setStorage(StorageKeys.isMuted, newVal);
  });

  watch(
    effectiveGains,
    (newVal) => {
      newVal.forEach((dbValue, index) => {
        audioEngine.setEQBand(index, dbValue);
      });
    },
    { deep: true, immediate: true },
  );

  watch(isPlaying, updateTaskbarState);
  watch(currentSong, updateTaskbarState);

  // --- EVENT LISTENERS ---
  audio.addEventListener('seeking', () => {
    isSeeking = true;
  });

  audio.addEventListener('seeked', () => {
    isSeeking = false;
    updateCurrentTime();
  });

  audio.addEventListener('timeupdate', () => {
    if (isSeeking || isInitialLoad) return;
    updateCurrentTime();
    if (Math.abs(audio.currentTime - lastSavedTime) >= 1) {
      setStorage(StorageKeys.currentTime, audio.currentTime);
      lastSavedTime = audio.currentTime;
    }
  });

  audio.addEventListener('durationchange', () => {
    if (!isNaN(audio.duration)) {
      duration.value = audio.duration;
    }
  });

  audio.addEventListener('ended', () => {
    if (isRepeat.value) {
      audio.currentTime = 0;
      audio.play().catch((err) => console.error('Error replaying song:', err));
    } else {
      nextTrack();
    }
  });

  // --- ACTIONS ON LOAD ---
  audioEngine.setVolume(volume.value / 100);
  audio.muted = isMuted.value;

  if (currentSong.value) {
    try {
      const srcUrl = convertFileSrc(currentSong.value.file_path);
      const onCanPlay = () => {
        audio.currentTime = currentTime.value;
        duration.value = audio.duration || currentSong.value?.duration || 0;
        audio.removeEventListener('canplay', onCanPlay);
        nextTick(() => {
          isInitialLoad = false;
        });
      };
      audio.addEventListener('canplay', onCanPlay);
      setTimeout(() => {
        if (isInitialLoad) isInitialLoad = false;
      }, 3000);

      audio.src = srcUrl;
      audio.load();
    } catch (err) {
      console.error('Error loading initial song:', err);
      isInitialLoad = false;
    }
  }

  async function playSongFromPath(filePath: string) {
    try {
      console.log('[Player] playSongFromPath:', filePath);
      const song = await invoke<Song>('get_song_by_path', { filePath });
      await playSong(song);
    } catch (err) {
      console.error('Failed to play song from path:', err);
    }
  }

  listen<any>('open-file', (event) => {
    const args = event.payload.args;
    console.log('[Player] open-file event received args:', args);
    const audioFile = args.find(
      (arg: string) =>
        arg.endsWith('.mp3') ||
        arg.endsWith('.flac') ||
        arg.endsWith('.wav') ||
        arg.endsWith('.m4a') ||
        arg.endsWith('.ogg') ||
        arg.endsWith('.m4r'),
    );
    if (audioFile) {
      playSongFromPath(audioFile);
    }
  });

  initTaskbarControls();

  // ==========================================
  // 4. RETURN STATEMENT
  // ==========================================
  return {
    currentSong,
    historyPlaylist,
    queuePlaylist,
    originalPlaylist,
    playlist,
    currentIndex,
    isPlaying,
    currentTime,
    duration,
    volume,
    isMuted,
    isShuffle,
    isRepeat,
    seekStep,
    volumeStep,
    eqGains,
    bassBoost,
    currentPresetName,
    playSong,
    togglePlay,
    setPlaying,
    seek,
    nextTrack,
    prevTrack,
    toggleShuffle,
    toggleRepeat,
    updateLyrics,
    updateTimeline,
    updateSongMetadata,
    playSongFromPath,
  };
});
