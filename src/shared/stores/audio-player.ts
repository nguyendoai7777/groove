import { ref, watch, computed } from 'vue'
import { defineStore } from 'pinia'
import { convertFileSrc } from '@tauri-apps/api/core'
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

  const getInitialPlaylist = () => {
    try {
      const val = localStorage.getItem('playlist')
      return val ? (JSON.parse(val) as PlaybackSong[]) : []
    } catch {
      return []
    }
  }

  const getInitialCurrentIndex = () => {
    try {
      const val = localStorage.getItem('currentIndex')
      if (val === null || val === '') return -1
      const num = Number(val)
      return isNaN(num) ? -1 : num
    } catch {
      return -1
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
  const playlist = ref(getInitialPlaylist())
  const currentIndex = ref(getInitialCurrentIndex())
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

  const shuffleHistory = ref<number[]>([])
  const shuffleHistoryIndex = ref(-1)

  watch(
    () => isShuffle.value,
    (newVal) => {
      try {
        localStorage.setItem('isShuffle', String(newVal))
      } catch (e) {
        console.error('Failed to save isShuffle to localStorage:', e)
      }
      if (newVal) {
        isRepeat.value = false
        if (currentIndex.value !== -1) {
          shuffleHistory.value = [currentIndex.value]
          shuffleHistoryIndex.value = 0
        } else {
          shuffleHistory.value = []
          shuffleHistoryIndex.value = -1
        }
      } else {
        shuffleHistory.value = []
        shuffleHistoryIndex.value = -1
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
    playlist,
    (newVal) => {
      try {
        // Strip out base64 thumbnail string to prevent QuotaExceededError (localStorage limit is 5MB)
        const cleanPlaylist = newVal.map(({ thumbnail, ...rest }) => rest)
        localStorage.setItem('playlist', JSON.stringify(cleanPlaylist))
      } catch (e) {
        console.error('Failed to save playlist to localStorage:', e)
      }
      if (isShuffle.value) {
        shuffleHistory.value = currentIndex.value !== -1 ? [currentIndex.value] : []
        shuffleHistoryIndex.value = currentIndex.value !== -1 ? 0 : -1
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
          // Strip out base64 thumbnail string to prevent QuotaExceededError
          const { thumbnail, ...rest } = newVal
          localStorage.setItem('currentSong', JSON.stringify(rest))
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
      audio.src = srcUrl
      audio.load()

      const onLoadedMetadata = () => {
        audio.currentTime = currentTime.value
        duration.value = audio.duration || currentSong.value?.duration || 0
        audio.removeEventListener('loadedmetadata', onLoadedMetadata)
      }
      audio.addEventListener('loadedmetadata', onLoadedMetadata)
    } catch (err) {
      console.error('Error loading initial song:', err)
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
    if (isSeeking) return
    updateCurrentTime()
    // Save to localStorage at most once per second to avoid performance issues
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
      const isLastSong = currentIndex.value >= playlist.value.length - 1

      if (isShuffle.value) {
        const playedSet = new Set(shuffleHistory.value)
        const unplayedIndices = playlist.value.map((_, index) => index).filter((index) => !playedSet.has(index))
        const allPlayed = unplayedIndices.length === 0 && shuffleHistoryIndex.value >= shuffleHistory.value.length - 1

        if (allPlayed) {
          isPlaying.value = false
          audio.currentTime = 0
          try {
            localStorage.setItem('currentTime', '0')
          } catch (e) {
            console.error('Failed to save currentTime to localStorage:', e)
          }
          lastSavedTime = 0
          shuffleHistory.value = []
          shuffleHistoryIndex.value = -1
          return
        }
      } else if (isLastSong) {
        isPlaying.value = false
        audio.currentTime = 0
        try {
          localStorage.setItem('currentTime', '0')
        } catch (e) {
          console.error('Failed to save currentTime to localStorage:', e)
        }
        lastSavedTime = 0
        return
      }

      nextTrack()
    }
  })

  // Play a song
  async function playSong(song: PlaybackSong, newPlaylist?: PlaybackSong[]) {
    if (currentSong.value?.id === song.id) {
      if (!isPlaying.value) {
        try {
          audioEngine.resume()
          await audio.play()
          isPlaying.value = true
        } catch (err) {
          console.error('Playback error:', err)
          isPlaying.value = false
        }
      }
      return
    }

    if (newPlaylist && newPlaylist.length > 0) {
      playlist.value = newPlaylist
      const idx = newPlaylist.findIndex((s) => s.id === song.id)
      currentIndex.value = idx !== -1 ? idx : 0
    } else {
      const idx = playlist.value.findIndex((s) => s.id === song.id)
      if (idx !== -1) {
        currentIndex.value = idx
      } else {
        playlist.value.push(song)
        currentIndex.value = playlist.value.length - 1
      }
    }

    if (isShuffle.value) {
      const expectedIndex = shuffleHistory.value[shuffleHistoryIndex.value]
      if (currentIndex.value !== expectedIndex) {
        shuffleHistory.value = [currentIndex.value]
        shuffleHistoryIndex.value = 0
      }
    }

    currentSong.value = song
    currentTime.value = 0
    duration.value = song.duration
    try {
      localStorage.setItem('currentTime', '0')
    } catch (e) {
      console.error('Failed to save currentTime to localStorage:', e)
    }
    lastSavedTime = 0

    try {
      const srcUrl = convertFileSrc(song.file_path)
      audio.src = srcUrl
      audio.load()
      audioEngine.resume()
      await audio.play()
      isPlaying.value = true
    } catch (err) {
      console.error('Playback error:', err)
      isPlaying.value = false
    }
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
    if (playlist.value.length === 0) return

    if (isShuffle.value) {
      if (shuffleHistoryIndex.value < shuffleHistory.value.length - 1) {
        shuffleHistoryIndex.value++
        currentIndex.value = shuffleHistory.value[shuffleHistoryIndex.value]
      } else {
        const playedSet = new Set(shuffleHistory.value)
        const unplayedIndices = playlist.value.map((_, index) => index).filter((index) => !playedSet.has(index))

        if (unplayedIndices.length > 0) {
          const randomIndex = unplayedIndices[Math.floor(Math.random() * unplayedIndices.length)]
          shuffleHistory.value.push(randomIndex)
          shuffleHistoryIndex.value = shuffleHistory.value.length - 1
          currentIndex.value = randomIndex
        } else {
          // All songs played, start a new shuffle cycle
          const randomIndex = Math.floor(Math.random() * playlist.value.length)
          shuffleHistory.value = [randomIndex]
          shuffleHistoryIndex.value = 0
          currentIndex.value = randomIndex
        }
      }
    } else {
      if (currentIndex.value < playlist.value.length - 1) {
        currentIndex.value++
      } else {
        currentIndex.value = 0
      }
    }

    const nextSong = playlist.value[currentIndex.value]
    if (nextSong) {
      playSong(nextSong)
    }
  }

  function prevTrack() {
    if (playlist.value.length === 0) return

    if (isShuffle.value) {
      if (shuffleHistoryIndex.value > 0) {
        shuffleHistoryIndex.value--
        currentIndex.value = shuffleHistory.value[shuffleHistoryIndex.value]
      } else {
        // At start of shuffle history, restart current song
        seek(0)
        return
      }
    } else {
      if (currentIndex.value > 0) {
        currentIndex.value--
      } else {
        currentIndex.value = playlist.value.length - 1
      }
    }

    const prevSong = playlist.value[currentIndex.value]
    if (prevSong) {
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
    const idx = playlist.value.findIndex((s) => s.id === songId)
    if (idx !== -1) {
      playlist.value[idx].lyrics = lyrics
    }
  }

  function updateTimeline(songId: number, timeline: string) {
    if (currentSong.value && currentSong.value.id === songId) {
      currentSong.value.timeline = timeline
    }
    const idx = playlist.value.findIndex((s) => s.id === songId)
    if (idx !== -1) {
      playlist.value[idx].timeline = timeline
    }
  }

  return {
    currentSong,
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
  }
})
