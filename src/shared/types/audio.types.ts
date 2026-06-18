export interface Song {
  id: number
  title: string | null
  artist: string | null
  album_id: number | null
  folder_id: number
  file_path: string
  filename: string
  duration: number
  lyrics?: string | null
  timeline?: string | null
}
