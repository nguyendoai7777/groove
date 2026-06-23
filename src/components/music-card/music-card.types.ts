export type MusicCardType = 'album' | 'folder' | 'playlist'

export interface MusicCardProps {
  type: MusicCardType
  title: string
  subtitle?: string
  thumbnail?: string // Base64 or image URL
  songsCount?: number
  accentColor?: string // CSS color string (e.g. hex, rgb, hsl)
}
