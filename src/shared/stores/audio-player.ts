import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { Song } from '@groovex/types'
import { EStoreKey } from './stores.definition'

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
    const val = localStorage.getItem('currentIndex')
    if (val === null || val === '') return -1
    const num = Number(val)
    return isNaN(num) ? -1 : num
  }

  const getInitialCurrentTime = () => {
    const val = localStorage.getItem('currentTime')
    if (val === null || val === '') return 0
    const num = Number(val)
    return isNaN(num) ? 0 : num
  }

  const getInitialVolume = () => {
    const val = localStorage.getItem('volume')
    if (val === null || val === '') return 75
    const num = Number(val)
    return isNaN(num) ? 75 : num
  }

  const getInitialIsMuted = () => {
    return localStorage.getItem('isMuted') === 'true'
  }

  const getInitialIsShuffle = () => {
    return localStorage.getItem('isShuffle') === 'true'
  }

  const getInitialIsRepeat = () => {
    return localStorage.getItem('isRepeat') === 'true'
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

  const getInitialSeekStep = () => {
    const val = localStorage.getItem('seekStep')
    if (val === null || val === '') return 5
    const num = Number(val)
    return isNaN(num) || num <= 0 ? 5 : num
  }
  const seekStep = ref(getInitialSeekStep())

  watch(seekStep, (newVal) => {
    localStorage.setItem('seekStep', String(newVal))
  })

  const getInitialVolumeStep = () => {
    const val = localStorage.getItem('volumeStep')
    if (val === null || val === '') return 2
    const num = Number(val)
    return isNaN(num) || num <= 0 ? 2 : num
  }
  const volumeStep = ref(getInitialVolumeStep())

  watch(volumeStep, (newVal) => {
    localStorage.setItem('volumeStep', String(newVal))
  })

  const shuffleHistory = ref<number[]>([])
  const shuffleHistoryIndex = ref(-1)

  watch(
    () => isShuffle.value,
    (newVal) => {
      localStorage.setItem('isShuffle', String(newVal))
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
      localStorage.setItem('isRepeat', String(newVal))
      if (newVal && isShuffle.value) {
        isShuffle.value = false
      }
    },
  )

  watch(
    playlist,
    (newVal) => {
      localStorage.setItem('playlist', JSON.stringify(newVal))
      if (isShuffle.value) {
        shuffleHistory.value = currentIndex.value !== -1 ? [currentIndex.value] : []
        shuffleHistoryIndex.value = currentIndex.value !== -1 ? 0 : -1
      }
    },
    { deep: true },
  )

  watch(currentIndex, (newVal) => {
    localStorage.setItem('currentIndex', String(newVal))
  })

  watch(currentSong, (newVal) => {
    if (newVal) {
      localStorage.setItem('currentSong', JSON.stringify(newVal))
    } else {
      localStorage.removeItem('currentSong')
    }
  })

  // HTML5 Audio element
  const audio = new Audio()
  audio.volume = volume.value / 100
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

  // Sync volume & mute state
  watch(volume, (newVal) => {
    audio.volume = newVal / 100
    localStorage.setItem('volume', String(newVal))
  })

  watch(isMuted, (newVal) => {
    audio.muted = newVal
    localStorage.setItem('isMuted', String(newVal))
  })

  // Sync playback progress from Audio element
  let lastSavedTime = currentTime.value
  audio.addEventListener('timeupdate', () => {
    currentTime.value = audio.currentTime
    // Save to localStorage at most once per second to avoid performance issues
    if (Math.abs(audio.currentTime - lastSavedTime) >= 1) {
      localStorage.setItem('currentTime', String(audio.currentTime))
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
          localStorage.setItem('currentTime', '0')
          lastSavedTime = 0
          shuffleHistory.value = []
          shuffleHistoryIndex.value = -1
          return
        }
      } else if (isLastSong) {
        isPlaying.value = false
        audio.currentTime = 0
        localStorage.setItem('currentTime', '0')
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
    localStorage.setItem('currentTime', '0')
    lastSavedTime = 0

    try {
      const srcUrl = convertFileSrc(song.file_path)
      audio.src = srcUrl
      audio.load()
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
      localStorage.setItem('currentTime', String(audio.currentTime))
      lastSavedTime = audio.currentTime
    } else {
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
      localStorage.setItem('currentTime', String(audio.currentTime))
      lastSavedTime = audio.currentTime
    }
  }

  function seek(seconds: number) {
    if (!currentSong.value) return
    audio.currentTime = seconds
    currentTime.value = seconds
    localStorage.setItem('currentTime', String(seconds))
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
    playSong,
    togglePlay,
    setPlaying,
    seek,
    nextTrack,
    prevTrack,
    toggleShuffle,
    toggleRepeat,
    updateLyrics,
  }
})
