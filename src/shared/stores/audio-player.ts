import { ref, watch, computed, nextTick } from 'vue'
import { defineStore } from 'pinia'
import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import {
  initialize as initTaskbar,
  isSupported as isTaskbarSupported,
  setNavigationEnabled as setTaskbarNavigation,
  setPlaybackState as setTaskbarPlaybackState,
} from 'tauri-plugin-taskbar'
import type { Song } from '@groovex/types'
import { EStoreKey } from './stores.definition'
import { AudioEngine } from '@groovex/core'

export interface PlaybackSong extends Song {
  thumbnail?: string
}

export const useAudioPlayer = defineStore(EStoreKey.Player, () => {
  const getInitialCurrentSong = () => {
    try {
      const val = localStorage.getItem('currentSong')
      return val ? (JSON.parse(val) as PlaybackSong) : null
    } catch {
      return null
    }
  }

  const getInitialCurrentTime = () => {
    try {
      const val = localStorage.getItem('currentTime')
      if (val === null || val === '') return 0
      const num = Number(val)
      return isNaN(num) ? 0 : num
    } catch {
      return 0
    }
  }

  const getInitialVolume = () => {
    try {
      const val = localStorage.getItem('volume')
      if (val === null || val === '') return 75
      const num = Number(val)
      return isNaN(num) ? 75 : num
    } catch {
      return 75
    }
  }

  const getInitialIsMuted = () => {
    try {
      return localStorage.getItem('isMuted') === 'true'
    } catch {
      return false
    }
  }

  const getInitialIsShuffle = () => {
    try {
      return localStorage.getItem('isShuffle') === 'true'
    } catch {
      return false
    }
  }

  const getInitialIsRepeat = () => {
    try {
      return localStorage.getItem('isRepeat') === 'true'
    } catch {
      return false
    }
  }

  const getInitialEQGains = () => {
    try {
      const val = localStorage.getItem('eqGains')
      if (val) {
        const arr = JSON.parse(val)
        if (Array.isArray(arr) && arr.length === 10) {
          return arr.map(Number)
        }
      }
    } catch {}
    return [0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
  }

  const getInitialBassBoost = () => {
    try {
      const val = localStorage.getItem('bassBoost')
      if (val !== null && val !== '') {
        const num = Number(val)
        return isNaN(num) ? 0 : num
      }
    } catch {}
    return 0
  }

  const getInitialPresetName = () => {
    try {
      return localStorage.getItem('currentPresetName') || 'Flat'
    } catch {}
    return 'Flat'
  }

  const currentSong = ref(getInitialCurrentSong())

  const getInitialHistoryPlaylist = () => {
    try {
      const val = localStorage.getItem('historyPlaylist')
      if (val) return JSON.parse(val) as PlaybackSong[]

      // Legacy migration
      const valLegacy = localStorage.getItem('playlist')
      const playlistLegacy = valLegacy ? (JSON.parse(valLegacy) as PlaybackSong[]) : []
      const currentIndexLegacy = Number(localStorage.getItem('currentIndex') || -1)
      if (playlistLegacy.length > 0 && currentIndexLegacy > 0) {
        return playlistLegacy.slice(0, currentIndexLegacy)
      }
    } catch {}
    return []
  }

  const getInitialQueuePlaylist = () => {
    try {
      const val = localStorage.getItem('queuePlaylist')
      if (val) return JSON.parse(val) as PlaybackSong[]

      // Legacy migration
      const valLegacy = localStorage.getItem('playlist')
      const playlistLegacy = valLegacy ? (JSON.parse(valLegacy) as PlaybackSong[]) : []
      const currentIndexLegacy = Number(localStorage.getItem('currentIndex') || -1)
      if (playlistLegacy.length > 0 && currentIndexLegacy !== -1) {
        return playlistLegacy.slice(currentIndexLegacy + 1)
      }
    } catch {}
    return []
  }

  const historyPlaylist = ref<PlaybackSong[]>(getInitialHistoryPlaylist())
  const queuePlaylist = ref<PlaybackSong[]>(getInitialQueuePlaylist())

  const getInitialOriginalPlaylist = () => {
    try {
      const val = localStorage.getItem('originalPlaylist')
      return val ? (JSON.parse(val) as PlaybackSong[]) : []
    } catch {
      return []
    }
  }

  const originalPlaylist = ref<PlaybackSong[]>(getInitialOriginalPlaylist())

  watch(
    originalPlaylist,
    (newVal) => {
      try {
        const cleanPlaylist = newVal.map(({ thumbnail, ...rest }) => rest)
        localStorage.setItem('originalPlaylist', JSON.stringify(cleanPlaylist))
      } catch (e) {
        console.error('Failed to save originalPlaylist to localStorage:', e)
      }
    },
    { deep: true },
  )

  function shuffleArray<T>(array: T[]): T[] {
    console.log('[Shuffle] Shuffling array of size:', array.length)
    const arr = [...array]
    for (let i = arr.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1))
      const temp = arr[i]
      arr[i] = arr[j]
      arr[j] = temp
    }
    console.log(
      '[Shuffle] Shuffled first 3 items:',
      arr.slice(0, 3).map((s: any) => s.title || s.filename),
    )
    return arr
  }

  const playlist = computed<PlaybackSong[]>(() => {
    const list: PlaybackSong[] = []
    list.push(...historyPlaylist.value)
    if (currentSong.value) {
      list.push(currentSong.value)
    }
    list.push(...queuePlaylist.value)
    return list
  })

  const currentIndex = computed<number>(() => {
    if (!currentSong.value) return -1
    return historyPlaylist.value.length
  })

  const isPlaying = ref(false)
  const currentTime = ref(getInitialCurrentTime())
  const duration = ref(currentSong.value?.duration || 0)
  const volume = ref(getInitialVolume())
  const isMuted = ref(getInitialIsMuted())
  const isShuffle = ref(getInitialIsShuffle())
  const isRepeat = ref(getInitialIsRepeat())
  const eqGains = ref<number[]>(getInitialEQGains())
  const bassBoost = ref<number>(getInitialBassBoost())
  const currentPresetName = ref<string>(getInitialPresetName())

  const getInitialSeekStep = () => {
    try {
      const val = localStorage.getItem('seekStep')
      if (val === null || val === '') return 5
      const num = Number(val)
      return isNaN(num) || num <= 0 ? 5 : num
    } catch {
      return 5
    }
  }
  const seekStep = ref(getInitialSeekStep())

  watch(seekStep, (newVal) => {
    try {
      localStorage.setItem('seekStep', String(newVal))
    } catch (e) {
      console.error('Failed to save seekStep to localStorage:', e)
    }
  })

  const getInitialVolumeStep = () => {
    try {
      const val = localStorage.getItem('volumeStep')
      if (val === null || val === '') return 2
      const num = Number(val)
      return isNaN(num) || num <= 0 ? 2 : num
    } catch {
      return 2
    }
  }
  const volumeStep = ref(getInitialVolumeStep())

  watch(volumeStep, (newVal) => {
    try {
      localStorage.setItem('volumeStep', String(newVal))
    } catch (e) {
      console.error('Failed to save volumeStep to localStorage:', e)
    }
  })

  watch(
    () => isShuffle.value,
    (newVal) => {
      console.log('[Player] isShuffle watch triggered. newVal =', newVal)
      try {
        localStorage.setItem('isShuffle', String(newVal))
      } catch (e) {
        console.error('Failed to save isShuffle to localStorage:', e)
      }
      if (newVal) {
        isRepeat.value = false
      }
    },
  )

  watch(
    () => isRepeat.value,
    (newVal) => {
      try {
        localStorage.setItem('isRepeat', String(newVal))
      } catch (e) {
        console.error('Failed to save isRepeat to localStorage:', e)
      }
      if (newVal && isShuffle.value) {
        isShuffle.value = false
      }
    },
  )

  watch(
    historyPlaylist,
    (newVal) => {
      try {
        const cleanPlaylist = newVal.map(({ thumbnail, ...rest }) => rest)
        localStorage.setItem('historyPlaylist', JSON.stringify(cleanPlaylist))
      } catch (e) {
        console.error('Failed to save historyPlaylist to localStorage:', e)
      }
    },
    { deep: true },
  )

  watch(
    queuePlaylist,
    (newVal) => {
      try {
        const cleanPlaylist = newVal.map(({ thumbnail, ...rest }) => rest)
        localStorage.setItem('queuePlaylist', JSON.stringify(cleanPlaylist))
      } catch (e) {
        console.error('Failed to save queuePlaylist to localStorage:', e)
      }
    },
    { deep: true },
  )

  watch(
    playlist,
    (newVal) => {
      try {
        const cleanPlaylist = newVal.map(({ thumbnail, ...rest }) => rest)
        localStorage.setItem('playlist', JSON.stringify(cleanPlaylist))
      } catch (e) {
        console.error('Failed to save playlist to localStorage:', e)
      }
    },
    { deep: true },
  )

  watch(currentIndex, (newVal) => {
    try {
      localStorage.setItem('currentIndex', String(newVal))
    } catch (e) {
      console.error('Failed to save currentIndex to localStorage:', e)
    }
  })

  let lastSongId = currentSong.value?.id || null
  watch(
    currentSong,
    (newVal) => {
      if (!newVal || newVal.id !== lastSongId) {
        targetSeekTime = null
        isSeeking = false
        lastSongId = newVal?.id || null
      }
      if (newVal) {
        try {
          localStorage.setItem('currentSong', JSON.stringify(newVal))
        } catch (e) {
          console.error('Failed to save currentSong to localStorage:', e)
        }
      } else {
        try {
          localStorage.removeItem('currentSong')
        } catch (e) {
          console.error('Failed to remove currentSong from localStorage:', e)
        }
      }
    },
    { deep: true },
  )

  // HTML5 Audio element
  const audio = new Audio()
  audio.crossOrigin = 'anonymous'
  const audioEngine = new AudioEngine(audio)
  audioEngine.setVolume(volume.value / 100)
  audio.muted = isMuted.value

  // Load initial source if there is a current song saved
  if (currentSong.value) {
    try {
      const srcUrl = convertFileSrc(currentSong.value.file_path)

      const onCanPlay = () => {
        audio.currentTime = currentTime.value
        duration.value = audio.duration || currentSong.value?.duration || 0
        audio.removeEventListener('canplay', onCanPlay)
        nextTick(() => {
          isInitialLoad = false
        })
      }
      audio.addEventListener('canplay', onCanPlay)

      // Fallback timeout to ensure isInitialLoad is cleared in case load fails
      setTimeout(() => {
        if (isInitialLoad) {
          isInitialLoad = false
        }
      }, 3000)

      audio.src = srcUrl
      audio.load()
    } catch (err) {
      console.error('Error loading initial song:', err)
      isInitialLoad = false
    }
  }

  // Keep track of effective gains applied to audio engine (incorporating bass boost)
  const effectiveGains = computed(() => {
    return eqGains.value.map((gain, idx) => {
      let offset = 0
      if (idx === 0)
        offset = bassBoost.value * 1.0 // 31Hz
      else if (idx === 1)
        offset = bassBoost.value * 0.8 // 63Hz
      else if (idx === 2)
        offset = bassBoost.value * 0.5 // 125Hz
      else if (idx === 3) offset = bassBoost.value * 0.2 // 250Hz
      return Math.max(-10, Math.min(10, gain + offset))
    })
  })

  // Watch effective gains and update the audio engine
  watch(
    effectiveGains,
    (newVal) => {
      newVal.forEach((dbValue, index) => {
        audioEngine.setEQBand(index, dbValue)
      })
    },
    { deep: true, immediate: true },
  )

  // Watch for state persistence
  watch(
    eqGains,
    (newVal) => {
      try {
        localStorage.setItem('eqGains', JSON.stringify(newVal))
      } catch (e) {
        console.error('Failed to save eqGains to localStorage:', e)
      }
    },
    { deep: true },
  )

  watch(bassBoost, (newVal) => {
    try {
      localStorage.setItem('bassBoost', String(newVal))
    } catch (e) {
      console.error('Failed to save bassBoost to localStorage:', e)
    }
  })

  watch(currentPresetName, (newVal) => {
    try {
      localStorage.setItem('currentPresetName', newVal)
    } catch (e) {
      console.error('Failed to save currentPresetName to localStorage:', e)
    }
  })

  // Sync volume & mute state
  watch(volume, (newVal) => {
    audioEngine.setVolume(newVal / 100)
    try {
      localStorage.setItem('volume', String(newVal))
    } catch (e) {
      console.error('Failed to save volume to localStorage:', e)
    }
  })

  watch(isMuted, (newVal) => {
    audio.muted = newVal
    try {
      localStorage.setItem('isMuted', String(newVal))
    } catch (e) {
      console.error('Failed to save isMuted to localStorage:', e)
    }
  })

  // Sync playback progress from Audio element
  let isSeeking = false
  let isInitialLoad = !!currentSong.value
  let targetSeekTime: number | null = null
  let lastSavedTime = currentTime.value

  function updateCurrentTime() {
    if (targetSeekTime !== null) {
      const diff = audio.currentTime - targetSeekTime
      if (diff >= 0) {
        currentTime.value = audio.currentTime
        targetSeekTime = null
      } else if (diff >= -1.5) {
        currentTime.value = targetSeekTime
      } else {
        currentTime.value = audio.currentTime
        targetSeekTime = null
      }
    } else {
      currentTime.value = audio.currentTime
    }
  }

  audio.addEventListener('seeking', () => {
    isSeeking = true
  })

  audio.addEventListener('seeked', () => {
    isSeeking = false
    updateCurrentTime()
  })

  audio.addEventListener('timeupdate', () => {
    if (isSeeking || isInitialLoad) return
    updateCurrentTime()
    if (Math.abs(audio.currentTime - lastSavedTime) >= 1) {
      try {
        localStorage.setItem('currentTime', String(audio.currentTime))
      } catch (e) {
        console.error('Failed to save currentTime to localStorage:', e)
      }
      lastSavedTime = audio.currentTime
    }
  })

  audio.addEventListener('durationchange', () => {
    if (!isNaN(audio.duration)) {
      duration.value = audio.duration
    }
  })

  audio.addEventListener('ended', () => {
    if (isRepeat.value) {
      audio.currentTime = 0
      audio.play().catch((err) => console.error('Error replaying song:', err))
    } else {
      nextTrack()
    }
  })

  // Load and play song audio
  async function loadAndPlay(song: PlaybackSong) {
    currentSong.value = song
    currentTime.value = 0
    duration.value = song.duration
    try {
      localStorage.setItem('currentTime', '0')
    } catch (e) {
      console.error('Failed to save currentTime to localStorage:', e)
    }
    lastSavedTime = 0

    // Fetch song-specific metadata dynamically (to get the real embedded thumbnail if any)
    invoke<any>('get_song_metadata', { songId: song.id })
      .then((meta) => {
        if (meta && meta.thumbnail && currentSong.value && currentSong.value.id === song.id) {
          currentSong.value.thumbnail = meta.thumbnail
        }
      })
      .catch((err) => {
        console.error('Failed to get song metadata during loadAndPlay:', err)
      })

    try {
      const srcUrl = convertFileSrc(song.file_path)
      audio.src = srcUrl
      audio.load()
      audioEngine.resume()
      await audio.play()
      isPlaying.value = true
    } catch (err: any) {
      if (err.name !== 'AbortError') {
        console.error('Playback error:', err)
      }
      isPlaying.value = false
    }
  }

  // Play a song
  async function playSong(song: PlaybackSong, newPlaylist?: PlaybackSong[]) {
    if (currentSong.value?.id === song.id) {
      if (!isPlaying.value) {
        try {
          audioEngine.resume()
          await audio.play()
          isPlaying.value = true
        } catch (err: any) {
          if (err.name !== 'AbortError') {
            console.error('Playback error:', err)
          }
          isPlaying.value = false
        }
      }
      return
    }

    const oldSong = currentSong.value

    console.log('[Player] playSong. song =', song.title || song.filename, 'hasNewPlaylist =', !!newPlaylist, 'isShuffle =', isShuffle.value)
    if (newPlaylist && newPlaylist.length > 0) {
      originalPlaylist.value = newPlaylist
    }

    // Always keep history and queue sequential in originalPlaylist
    if (originalPlaylist.value.length > 0) {
      const idx = originalPlaylist.value.findIndex((s) => s.id === song.id)
      if (idx !== -1) {
        queuePlaylist.value = originalPlaylist.value.slice(idx + 1)
        historyPlaylist.value = originalPlaylist.value.slice(0, idx)
      } else {
        queuePlaylist.value = originalPlaylist.value.filter((s) => s.id !== song.id)
        historyPlaylist.value = []
      }
    } else {
      queuePlaylist.value = []
      historyPlaylist.value = []
    }

    await loadAndPlay(song)
  }

  function togglePlay() {
    if (!currentSong.value) return

    if (isPlaying.value) {
      audio.pause()
      isPlaying.value = false
      try {
        localStorage.setItem('currentTime', String(audio.currentTime))
      } catch (e) {
        console.error('Failed to save currentTime to localStorage:', e)
      }
      lastSavedTime = audio.currentTime
    } else {
      audioEngine.resume()
      audio
        .play()
        .then(() => {
          isPlaying.value = true
        })
        .catch((err) => {
          console.error('Playback failed:', err)
          isPlaying.value = false
        })
    }
  }

  function setPlaying(state: boolean) {
    if (!currentSong.value) return
    if (state) {
      audioEngine.resume()
      audio
        .play()
        .then(() => {
          isPlaying.value = true
        })
        .catch((err) => {
          console.error('Playback failed:', err)
          isPlaying.value = false
        })
    } else {
      audio.pause()
      isPlaying.value = false
      try {
        localStorage.setItem('currentTime', String(audio.currentTime))
      } catch (e) {
        console.error('Failed to save currentTime to localStorage:', e)
      }
      lastSavedTime = audio.currentTime
    }
  }

  function seek(seconds: number) {
    if (!currentSong.value) return
    isSeeking = true
    targetSeekTime = seconds
    audio.currentTime = seconds
    currentTime.value = seconds
    try {
      localStorage.setItem('currentTime', String(seconds))
    } catch (e) {
      console.error('Failed to save currentTime to localStorage:', e)
    }
    lastSavedTime = seconds
  }

  function nextTrack() {
    console.log('[Player] nextTrack called. Queue length:', queuePlaylist.value.length, 'isShuffle =', isShuffle.value)
    if (originalPlaylist.value.length === 0) return

    const oldSong = currentSong.value
    let nextSong: PlaybackSong | null = null

    if (isShuffle.value && originalPlaylist.value.length > 1) {
      const candidates = originalPlaylist.value.filter((s) => s.id !== oldSong?.id)
      const randIdx = Math.floor(Math.random() * candidates.length)
      nextSong = candidates[randIdx]
    } else {
      if (queuePlaylist.value.length > 0) {
        nextSong = queuePlaylist.value[0]
      } else if (isRepeat.value) {
        nextSong = originalPlaylist.value[0]
      }
    }

    if (nextSong) {
      console.log('[Player] playing next song:', nextSong.title || nextSong.filename)
      playSong(nextSong)
    } else {
      isPlaying.value = false
      audio.currentTime = 0
      try {
        localStorage.setItem('currentTime', '0')
      } catch (e) {
        console.error('Failed to save currentTime to localStorage:', e)
      }
      lastSavedTime = 0
    }
  }

  function prevTrack() {
    if (historyPlaylist.value.length === 0) {
      seek(0)
      return
    }

    const prevSong = historyPlaylist.value[historyPlaylist.value.length - 1]
    if (prevSong) {
      console.log('[Player] playing prev song:', prevSong.title || prevSong.filename)
      playSong(prevSong)
    }
  }

  function toggleShuffle() {
    isShuffle.value = !isShuffle.value
  }

  function toggleRepeat() {
    isRepeat.value = !isRepeat.value
  }

  function updateLyrics(songId: number, lyrics: string) {
    if (currentSong.value && currentSong.value.id === songId) {
      currentSong.value.lyrics = lyrics
    }
    const hIdx = historyPlaylist.value.findIndex((s) => s.id === songId)
    if (hIdx !== -1) {
      historyPlaylist.value[hIdx].lyrics = lyrics
    }
    const qIdx = queuePlaylist.value.findIndex((s) => s.id === songId)
    if (qIdx !== -1) {
      queuePlaylist.value[qIdx].lyrics = lyrics
    }
  }

  function updateTimeline(songId: number, timeline: string) {
    if (currentSong.value && currentSong.value.id === songId) {
      currentSong.value.timeline = timeline
    }
    const hIdx = historyPlaylist.value.findIndex((s) => s.id === songId)
    if (hIdx !== -1) {
      historyPlaylist.value[hIdx].timeline = timeline
    }
    const qIdx = queuePlaylist.value.findIndex((s) => s.id === songId)
    if (qIdx !== -1) {
      queuePlaylist.value[qIdx].timeline = timeline
    }
  }

  async function updateSongMetadata(
    songId: number,
    metadata: {
      filename: string
      title: string
      artist: string
      album_name: string
      track_number: string
      thumbnail: string
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
      })

      if (currentSong.value && currentSong.value.id === songId) {
        currentSong.value.title = updatedSong.title
        currentSong.value.artist = updatedSong.artist
        currentSong.value.album_id = updatedSong.album_id
        currentSong.value.file_path = updatedSong.file_path
        currentSong.value.filename = updatedSong.filename
        if (metadata.thumbnail) {
          currentSong.value.thumbnail = metadata.thumbnail
        }
      }

      const hIdx = historyPlaylist.value.findIndex((s) => s.id === songId)
      if (hIdx !== -1) {
        historyPlaylist.value[hIdx].title = updatedSong.title
        historyPlaylist.value[hIdx].artist = updatedSong.artist
        historyPlaylist.value[hIdx].album_id = updatedSong.album_id
        historyPlaylist.value[hIdx].file_path = updatedSong.file_path
        historyPlaylist.value[hIdx].filename = updatedSong.filename
        if (metadata.thumbnail) {
          historyPlaylist.value[hIdx].thumbnail = metadata.thumbnail
        }
      }

      const qIdx = queuePlaylist.value.findIndex((s) => s.id === songId)
      if (qIdx !== -1) {
        queuePlaylist.value[qIdx].title = updatedSong.title
        queuePlaylist.value[qIdx].artist = updatedSong.artist
        queuePlaylist.value[qIdx].album_id = updatedSong.album_id
        queuePlaylist.value[qIdx].file_path = updatedSong.file_path
        queuePlaylist.value[qIdx].filename = updatedSong.filename
        if (metadata.thumbnail) {
          queuePlaylist.value[qIdx].thumbnail = metadata.thumbnail
        }
      }
      return updatedSong
    } catch (err) {
      console.error('Failed to update song metadata:', err)
      throw err
    }
  }

  // Taskbar media controls setup
  let taskbarSetupDone = false
  async function initTaskbarControls() {
    try {
      if (await isTaskbarSupported()) {
        await initTaskbar()
        taskbarSetupDone = true

        // Set initial state
        await setTaskbarPlaybackState(isPlaying.value)
        await setTaskbarNavigation(!!currentSong.value, !!currentSong.value)

        // Listen for taskbar clicks
        await listen('media-prev', () => {
          prevTrack()
        })
        await listen('media-toggle', () => {
          togglePlay()
        })
        await listen('media-next', () => {
          nextTrack()
        })
      }
    } catch (err) {
      console.error('Failed to initialize taskbar controls:', err)
    }
  }

  // Watchers to sync state with taskbar
  watch(
    () => isPlaying.value,
    async (newValue) => {
      if (taskbarSetupDone) {
        try {
          await setTaskbarPlaybackState(newValue)
        } catch (err) {
          console.error('Failed to update taskbar playback state:', err)
        }
      }
    },
  )

  watch(
    () => currentSong.value,
    async (newSong) => {
      if (taskbarSetupDone) {
        try {
          await setTaskbarNavigation(!!newSong, !!newSong)
          await setTaskbarPlaybackState(isPlaying.value)
        } catch (err) {
          console.error('Failed to update taskbar on song change:', err)
        }
      }
    },
  )

  // Initialize taskbar controls
  initTaskbarControls()

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
  }
})
