import { ref, watch } from 'vue'
import { defineStore } from 'pinia'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { Song } from '@groovex/types'
import { EStoreKey } from './stores.definition'

export interface PlaybackSong extends Song {
  thumbnail?: string
}

export const useAudioPlayer = defineStore(EStoreKey.Player, () => {
  const currentSong = ref<PlaybackSong | null>(null)
  const playlist = ref<PlaybackSong[]>([])
  const currentIndex = ref(-1)
  const isPlaying = ref(false)
  const currentTime = ref(0)
  const duration = ref(0)
  const volume = ref(75)
  const isMuted = ref(false)
  const isShuffle = ref(false)
  const isRepeat = ref(false)

  // HTML5 Audio element
  const audio = new Audio()
  audio.volume = volume.value / 100
  audio.muted = isMuted.value

  // Sync volume & mute state
  watch(volume, (newVal) => {
    audio.volume = newVal / 100
  })

  watch(isMuted, (newVal) => {
    audio.muted = newVal
  })

  // Sync playback progress from Audio element
  audio.addEventListener('timeupdate', () => {
    currentTime.value = audio.currentTime
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

    currentSong.value = song
    currentTime.value = 0
    duration.value = song.duration

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
    }
  }

  function seek(seconds: number) {
    if (!currentSong.value) return
    audio.currentTime = seconds
    currentTime.value = seconds
  }

  function nextTrack() {
    if (playlist.value.length === 0) return

    if (isShuffle.value) {
      const randomIndex = Math.floor(Math.random() * playlist.value.length)
      currentIndex.value = randomIndex
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

    if (currentIndex.value > 0) {
      currentIndex.value--
    } else {
      currentIndex.value = playlist.value.length - 1
    }

    const prevSong = playlist.value[currentIndex.value]
    if (prevSong) {
      playSong(prevSong)
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
    playSong,
    togglePlay,
    setPlaying,
    seek,
    nextTrack,
    prevTrack,
  }
})
