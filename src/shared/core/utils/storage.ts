export const StorageKeys = {
  currentSong: 'currentSong',
  currentTime: 'currentTime',
  volume: 'volume',
  isMuted: 'isMuted',
  isShuffle: 'isShuffle',
  isRepeat: 'isRepeat',
  eqGains: 'eqGains',
  bassBoost: 'bassBoost',
  currentPresetName: 'currentPresetName',
  historyPlaylist: 'historyPlaylist',
  queuePlaylist: 'queuePlaylist',
  originalPlaylist: 'originalPlaylist',
  playlist: 'playlist',
  currentIndex: 'currentIndex',
  seekStep: 'seekStep',
  volumeStep: 'volumeStep',
} as const

export type StorageKey = CastString<keyof typeof StorageKeys>

export const StorageLocal = {
  setItem: (key: StorageKey, data: unknown) => {
    if (typeof data === 'object' && data !== null) {
      localStorage.setItem(key, JSON.stringify(data))
    } else {
      localStorage.setItem(key, String(data))
    }
  },
  getItem: <TData = string>(key: StorageKey): (TData extends object ? TData : string) | null => {
    const item = localStorage.getItem(key)
    if (!item) {
      return null
    }
    try {
      const parsed = JSON.parse(item)
      if (parsed && typeof parsed === 'object') {
        return parsed
      }
      return item as unknown as TData extends object ? TData : string
    } catch (e) {
      return item as unknown as TData extends object ? TData : string
    }
  },
  removeItem: (key: StorageKey) => {
    localStorage.removeItem(key)
  },
  clear: () => {
    localStorage.clear()
  },
}
