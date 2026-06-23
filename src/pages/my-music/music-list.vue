<template>
  <div
    class="flex flex-col min-h-full text-theme-text select-none"
    :style="{
      '--accent-bg': accentBgColor,
    }">
    <!-- Background immersive glow -->
    <div class="sticky top-0 left-0 right-0 h-0 -z-10 pointer-events-none">
      <div
        class="absolute top-0 left-0 right-0 h-music-list-glow-h bg-linear-to-b from-(--accent-bg) to-transparent opacity-40 transition-all duration-500"></div>
    </div>

    <!-- Category Header Info -->
    <div
      class="sticky top-0 p-3 z-20 flex transition-all duration-300 ease-in-out"
      :class="[
        isScrolled
          ? 'flex-row items-center gap-4 py-3 backdrop-blur-md px-4'
          : 'flex-col md:flex-row gap-6 items-start md:items-end pb-2 mb-8',
      ]">
      <!-- Big cover image -->
      <div
        class="relative rounded-xl overflow-hidden bg-theme-bg-placeholder shadow-xl shrink-0 transition-all duration-300 ease-in-out"
        :class="isScrolled ? 'w-20 h-20 shadow-md' : 'w-40 h-40 shadow-xl'">
        <img v-if="thumbnail" :src="thumbnail" :alt="title" class="w-full h-full object-cover" />
        <div
          v-else
          class="w-full h-full flex items-center justify-center transition-all duration-300 ease-in-out"
          :class="placeholderClass"
          :style="type === 'playlist' ? { backgroundColor: accentColor || '#06b6d4' } : {}">
          <svg-sprite
            :src="type === 'folder' ? 'Play' : 'Album'"
            :class="[
              type === 'folder'
                ? 'text-theme-text-on-accent drop-shadow-lg'
                : type === 'playlist'
                  ? 'text-white/90! drop-shadow-md'
                  : 'text-theme-text-disabled',
              isScrolled ? 'w-8 h-8' : 'w-16 h-16',
            ]"
            class="transition-all duration-300 ease-in-out" />
        </div>
      </div>

      <!-- Text details -->
      <div class="flex-1 min-w-0">
        <span
          class="text-xs text-theme-accent-light font-bold tracking-widest transition-all duration-300 block"
          :class="isScrolled ? 'opacity-0 h-0 overflow-hidden mb-0' : 'mb-1'">
          {{ type === 'folder' ? 'Folder' : type === 'playlist' ? 'Playlist' : 'Album' }}
        </span>
        <h2
          class="font-extrabold truncate text-white transition-all duration-300"
          :class="isScrolled ? 'text-lg md:text-xl mt-0' : 'text-3xl md:text-4xl mt-1'">
          {{ title }}
        </h2>
        <p
          class="text-theme-text-muted flex items-center gap-2 transition-all duration-300 mt-1"
          :class="isScrolled ? 'text-xs md:text-sm' : 'text-sm md:mt-2'">
          <span v-if="!isScrolled" class="transition-opacity duration-300">
            {{ displaySubtitle }}
          </span>
          <span v-if="!isScrolled && songs.length > 0" class="text-theme-text-disabled">•</span>

          <span v-if="isScrolled" class="font-semibold text-theme-accent-light tracking-wider">
            {{ type === 'folder' ? 'Folder' : type === 'playlist' ? 'Playlist' : 'Album' }}
          </span>
          <span v-if="isScrolled" class="text-theme-text-disabled">•</span>

          <span v-if="songs.length > 0">{{ songs.length }} items</span>
        </p>

        <!-- Quick actions -->
        <div
          class="flex gap-3 transition-all duration-300 ease-in-out overflow-hidden"
          :class="isScrolled ? 'max-h-0 opacity-0 mt-0 pointer-events-none' : 'max-h-12 opacity-100 mt-4'">
          <custom-btn
            variant="primary"
            rounded="full"
            size="md"
            @click="handlePlayAll"
            :disabled="songs.length === 0"
            class="flex items-center gap-2">
            <svg-sprite src="Play" class="w-4 h-4" />
            Play all
          </custom-btn>
        </div>
      </div>
    </div>

    <!-- Songs Table -->
    <div class="flex-1 bg-theme-bg-card overflow-hidden">
      <!-- Loading State -->
      <div v-if="isLoading" class="flex flex-col items-center justify-center py-20 gap-3">
        <div class="w-8 h-8 rounded-full border-2 border-theme-accent/20 border-t-theme-accent animate-spin"></div>
        <span class="text-xs text-theme-text-disabled">Loading songs...</span>
      </div>

      <!-- Empty State -->
      <div v-else-if="songs.length === 0" class="flex flex-col items-center justify-center py-20 text-center">
        <svg-sprite src="Song" class="w-12 h-12 text-theme-text-disabled mb-3" />
        <span class="text-sm font-semibold text-theme-text-muted">This category is empty</span>
        <span class="text-xs text-theme-text-disabled mt-1">Scan folder to add songs</span>
      </div>

      <!-- Table Content -->
      <div v-else class="overflow-x-auto">
        <table class="w-full text-left border-collapse">
          <thead>
            <tr class="text-theme-text-disabled text-xs font-semibold tracking-wider">
              <th class="py-3.5 px-4 w-12 text-center">#</th>
              <th class="py-3.5 px-4">Name</th>
              <th class="py-3.5 px-4 w-24 text-left min-w-50">Duration</th>
              <th class="py-3.5 px-4 hidden md:table-cell">Artist</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="(song, idx) in songs"
              :key="song.id"
              :data-src="song.file_path"
              class="group/row hover:bg-gray-400/10 border-b border-theme-border/40 transition-colors duration-150 text-sm cursor-pointer"
              @click="handlePlaySong(song)"
              @contextmenu.prevent="onRowContextMenu($event, song)">
              <td class="py-3 px-4 text-center text-theme-text-disabled group-hover/row:text-theme-accent-light transition-colors w-12">
                <div class="relative flex items-center justify-center w-full h-full">
                  <!-- If this song is NOT currently selected/playing in the player -->
                  <template v-if="player.currentSong?.id !== song.id">
                    <!-- Default state: show index number -->
                    <span class="group-hover/row:hidden font-mono text-xs">{{ idx + 1 }}</span>
                    <!-- Hover state: show Play icon -->
                    <svg-sprite
                      src="Play"
                      class="w-3 h-3 mx-auto hidden group-hover/row:block text-theme-accent-light fill-theme-accent-light" />
                  </template>

                  <!-- If this song IS currently selected/playing in the player -->
                  <template v-else>
                    <!-- Default state: show bouncing visualizer SVG -->
                    <div class="group-hover/row:hidden flex items-center justify-center text-theme-accent-light">
                      <playing-visualizer :paused="!player.isPlaying" class="w-3.5 h-3.5" />
                    </div>
                    <!-- Hover state: show Play/Pause icon depending on state -->
                    <div class="hidden group-hover/row:block text-theme-accent-light">
                      <!-- If playing, show Pause icon -->
                      <svg
                        v-if="player.isPlaying"
                        class="w-3.5 h-3.5 fill-theme-accent-light mx-auto"
                        viewBox="0 0 24 24"
                        xmlns="http://www.w3.org/2000/svg">
                        <path d="M6 19h4V5H6v14zm8-14v14h4V5h-4z" />
                      </svg>
                      <!-- If paused, show Play icon -->
                      <svg-sprite v-else src="Play" class="w-3 h-3 mx-auto text-theme-accent-light fill-theme-accent-light" />
                    </div>
                  </template>
                </div>
              </td>

              <td
                class="py-3 px-4 font-medium truncate max-w-xs sm:max-w-sm transition-colors duration-150"
                :class="
                  player.currentSong?.id === song.id ? 'text-theme-accent-light' : 'text-theme-text-secondary group-hover/row:text-white'
                ">
                {{ song.filename.replace(/\.[^/.]+$/, '') }}
              </td>
              <td class="py-3 px-4 text-left text-theme-text-muted tabular-nums">
                {{ formatDuration(song.duration) }}
              </td>

              <td class="py-3 px-4 text-theme-text-muted hidden md:table-cell truncate max-w-xs">
                {{ song.artist || '—' }}
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>

    <!-- Programmatic Context Menu -->
    <div
      ref="contextMenuActivator"
      :style="{
        position: 'fixed',
        top: contextMenuY + 'px',
        left: contextMenuX + 'px',
        width: '1px',
        height: '1px',
        pointerEvents: 'none',
        zIndex: 9999,
      }">
      <v-menu v-model="showContextMenu" :close-on-content-click="true">
        <v-list class="bg-theme-bg-item border border-theme-border! text-xs py-1" density="compact">
          <!-- Add to Playlist Submenu -->
          <v-list-item class="cursor-pointer hover:bg-theme-bg-placeholder/20">
            <template #prepend>
              <svg-sprite src="Play" class="w-3.5 h-3.5 mr-2 text-theme-text-secondary" />
            </template>
            <v-list-item-title>Thêm vào danh sách phát</v-list-item-title>

            <!-- Nested Submenu -->
            <v-menu activator="parent" submenu open-on-hover location="end top">
              <v-list class="bg-theme-bg-item border border-theme-border! text-xs py-1" density="compact">
                <v-list-item
                  v-for="playlist in playlists"
                  :key="playlist.id"
                  class="cursor-pointer hover:bg-theme-bg-placeholder/20"
                  @click="addSongToPlaylist(playlist.id)">
                  <div class="flex items-center gap-2">
                    <span class="w-2.5 h-2.5 rounded-full shrink-0" :style="{ backgroundColor: playlist.color }"></span>
                    <span class="truncate max-w-40">{{ playlist.name }}</span>
                  </div>
                </v-list-item>
                <v-divider class="my-1 border-theme-border/30" v-if="playlists.length > 0"></v-divider>
                <v-list-item
                  class="cursor-pointer hover:bg-theme-bg-placeholder/20 text-theme-accent-light"
                  @click="openCreatePlaylistDialogFromMenu">
                  <template #prepend>
                    <svg-sprite src="Album" class="w-3 h-3 mr-1 text-theme-accent-light" />
                  </template>
                  <v-list-item-title>Tạo danh sách mới...</v-list-item-title>
                </v-list-item>
              </v-list>
            </v-menu>
          </v-list-item>

          <v-divider class="my-1 border-theme-border/30"></v-divider>

          <!-- Edit Timeline -->
          <v-list-item class="cursor-pointer hover:bg-theme-bg-placeholder/20" @click="openEditTimeline">
            <template #prepend>
              <svg-sprite src="Loop" class="w-3.5 h-3.5 mr-2 text-theme-text-secondary" />
            </template>
            <v-list-item-title>Edit timeline</v-list-item-title>
          </v-list-item>

          <!-- Edit Lyrics -->
          <v-list-item class="cursor-pointer hover:bg-theme-bg-placeholder/20" @click="openEditLyrics">
            <template #prepend>
              <svg-sprite src="Edit" class="w-3.5 h-3.5 mr-2 text-theme-text-secondary" />
            </template>
            <v-list-item-title>Edit lyrics</v-list-item-title>
          </v-list-item>

          <!-- Edit Metadata -->
          <v-list-item class="cursor-pointer hover:bg-theme-bg-placeholder/20" @click="openEditMetadata">
            <template #prepend>
              <svg-sprite src="Album" class="w-3.5 h-3.5 mr-2 text-theme-text-secondary" />
            </template>
            <v-list-item-title>Edit metadata</v-list-item-title>
          </v-list-item>

          <!-- If type is playlist, support removing song from playlist -->
          <template v-if="type === 'playlist'">
            <v-divider class="my-1 border-theme-border/30"></v-divider>
            <v-list-item class="cursor-pointer hover:bg-theme-bg-placeholder/20 text-red-400!" @click="removeSongFromPlaylist">
              <template #prepend>
                <svg-sprite src="Delete" class="w-3.5 h-3.5 mr-2 text-red-400" />
              </template>
              <v-list-item-title>Xóa khỏi danh sách phát</v-list-item-title>
            </v-list-item>
          </template>
        </v-list>
      </v-menu>
    </div>

    <!-- Dialog components -->
    <metadata-dialog v-model="isEditingMetadata" :song-id="selectedSongForMenu?.id" @saved="onMetadataSaved" />
    <timeline-dialog
      v-model="isEditingTimeline"
      :song-id="selectedSongForMenu?.id"
      :initial-timeline="selectedSongForMenu?.timeline"
      @saved="onTimelineSaved" />
    <lyrics-dialog v-model="isEditingLyrics" :song-id="selectedSongForMenu?.id" @saved="onLyricsSaved" />

    <!-- Create Playlist Dialog (triggered from track context menu) -->
    <v-dialog v-model="createDialogVisible" max-width="500">
      <v-card
        class="grx-ConfirmerCard bg-theme-bg-item! text-theme-text! border border-theme-border! rounded-xl! overflow-hidden shadow-2xl">
        <v-card-title class="px-4 py-3 text-base font-bold border-b border-theme-border/30 bg-theme-bg-placeholder/20">
          Create Playlist
        </v-card-title>
        <v-card-text class="px-4 py-4">
          <div class="flex flex-col gap-4">
            <label class="text-left">
              <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Playlist Name</div>
              <v-text-field
                v-model="newPlaylistName"
                placeholder="e.g. My Favorite Songs"
                density="compact"
                variant="outlined"
                color="cyan-accent-3"
                hide-details />
            </label>

            <label class="text-left">
              <div class="block mb-1.5 text-theme-text-secondary font-medium text-xs tracking-wide">Theme Color</div>
              <div class="flex gap-3 items-center">
                <input
                  type="color"
                  v-model="newPlaylistColor"
                  class="w-10 h-10 rounded-lg border border-theme-border/50 cursor-pointer overflow-hidden bg-transparent shrink-0" />
                <v-text-field
                  v-model="newPlaylistColor"
                  placeholder="#06b6d4"
                  density="compact"
                  variant="outlined"
                  color="cyan-accent-3"
                  hide-details
                  class="flex-1" />
              </div>
            </label>
          </div>
        </v-card-text>
        <v-card-actions class="px-4 py-3 flex justify-end gap-2 bg-theme-bg-placeholder/20 border-t border-theme-border/50">
          <custom-btn variant="secondary" @click="createDialogVisible = false" :disabled="isCreatingPlaylist">Cancel</custom-btn>
          <custom-btn variant="primary" @click="handleCreatePlaylist" :loading="isCreatingPlaylist">Create</custom-btn>
        </v-card-actions>
      </v-card>
    </v-dialog>
  </div>
</template>

<script setup lang="ts">
  import { ref, computed, onMounted, nextTick } from 'vue'
  import { invoke } from '@tauri-apps/api/core'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import PlayingVisualizer from '@groovex/ui/playing-visualizer/playing-visualizer.vue'
  import CustomBtn from '@groovex/ui/button/custom-btn.vue'
  import { useLayoutScroll } from '../../shared/composables/use-layout-scroll'
  import { formatDuration } from '@groovex/core'
  import { useAudioPlayer } from '@groovex/state'
  import type { Song } from '@groovex/types'

  // Dialog component imports
  import MetadataDialog from '@groovex/ui/song/metadata-dialog.vue'
  import TimelineDialog from '@groovex/ui/song/timeline-dialog.vue'
  import LyricsDialog from '@groovex/ui/song/lyrics-dialog.vue'
  import { useToast } from '@groovex/composables'

  const props = defineProps<{
    type: 'album' | 'folder' | 'playlist'
    id: number
    title: string
    subtitle?: string
    thumbnail?: string
    accentColor?: string
    songsCount?: number
  }>()

  defineEmits<{
    (e: 'back'): void
  }>()

  const songs = ref<Song[]>([])
  const isLoading = ref(true)

  const isScrolled = ref(false)

  useLayoutScroll((instance) => {
    const { viewport } = instance.elements()
    isScrolled.value = viewport.scrollTop > 40
  })

  const dynamicAccentColor = ref(props.accentColor || 'rgba(6, 182, 212, 0.4)')

  // Accent glow background color
  const accentBgColor = computed(() => {
    return dynamicAccentColor.value
  })

  // Subtitle string
  const displaySubtitle = computed(() => {
    if (props.subtitle) return props.subtitle
    if (props.type === 'folder') return 'Local Folder'
    if (props.type === 'playlist') return 'Playlist'
    return 'Unknown Artist'
  })

  // Fallback placeholder class
  const placeholderClass = computed(() => {
    if (props.type === 'folder') {
      return 'bg-gradient-to-br from-theme-accent to-theme-accent-secondary shadow-music-placeholder-inset'
    }
    if (props.type === 'playlist') {
      return 'shadow-md border border-theme-border/30'
    }
    return 'bg-theme-bg-placeholder border border-theme-border/50'
  })

  // Helper function to extract average dominant color from image URL or base64
  function getAverageColor(imageUrl: string): Promise<string> {
    return new Promise((resolve) => {
      const img = new Image()
      img.crossOrigin = 'Anonymous'
      img.onload = () => {
        const canvas = document.createElement('canvas')
        canvas.width = 1
        canvas.height = 1
        const ctx = canvas.getContext('2d')
        if (ctx) {
          ctx.drawImage(img, 0, 0, 1, 1)
          const pixel = ctx.getImageData(0, 0, 1, 1).data
          let r = pixel[0]
          let g = pixel[1]
          let b = pixel[2]

          // Keep brightness under control for dark mode text readability
          const brightness = (r * 299 + g * 587 + b * 114) / 1000
          if (brightness > 120) {
            const factor = 120 / brightness
            r = Math.round(r * factor)
            g = Math.round(g * factor)
            b = Math.round(b * factor)
          }
          resolve(`rgb(${r}, ${g}, ${b})`)
        } else {
          resolve('#06b6d4')
        }
      }
      img.onerror = () => resolve('#06b6d4')
      img.src = imageUrl
    })
  }

  // Playlists and context menu states
  const playlists = ref<any[]>([])
  const contextMenuX = ref(0)
  const contextMenuY = ref(0)
  const showContextMenu = ref(false)
  const selectedSongForMenu = ref<Song | null>(null)

  // Dialog states
  const isEditingMetadata = ref(false)
  const isEditingTimeline = ref(false)
  const isEditingLyrics = ref(false)

  // Create playlist from menu states
  const createDialogVisible = ref(false)
  const isCreatingPlaylist = ref(false)
  const newPlaylistName = ref('')
  const newPlaylistColor = ref('#06b6d4')

  const toast = useToast()

  async function loadPlaylists() {
    try {
      playlists.value = await invoke<any[]>('get_playlists')
    } catch (err) {
      console.error('Failed to load playlists:', err)
    }
  }

  // Fetch category songs on mount
  onMounted(async () => {
    if (props.thumbnail) {
      try {
        const avgColor = await getAverageColor(props.thumbnail)
        dynamicAccentColor.value = avgColor
      } catch (e) {
        console.error('Failed to extract dominant color:', e)
      }
    } else if (props.accentColor) {
      dynamicAccentColor.value = props.accentColor
    }

    try {
      if (props.type === 'playlist') {
        songs.value = await invoke<Song[]>('get_playlist_songs', {
          playlistId: props.id,
        })
      } else {
        songs.value = await invoke<Song[]>('get_category_songs', {
          categoryType: props.type,
          categoryId: props.id,
        })
      }
    } catch (err) {
      console.error('Error fetching songs:', err)
    } finally {
      isLoading.value = false
    }

    await loadPlaylists()
  })

  const player = useAudioPlayer()

  function handlePlayAll() {
    if (songs.value.length === 0) return
    const playlist = songs.value.map((s) => ({
      ...s,
      thumbnail: props.thumbnail,
    }))
    player.playSong(playlist[0], playlist)
  }

  function handlePlaySong(song: Song) {
    if (player.currentSong?.id === song.id) {
      player.togglePlay()
      return
    }
    const playlist = songs.value.map((s) => ({
      ...s,
      thumbnail: props.thumbnail,
    }))
    const playSongItem = playlist.find((s) => s.id === song.id) || {
      ...song,
      thumbnail: props.thumbnail,
    }
    player.playSong(playSongItem, playlist)
  }

  // Context Menu Actions
  function onRowContextMenu(e: MouseEvent, song: Song) {
    selectedSongForMenu.value = song
    contextMenuX.value = e.clientX
    contextMenuY.value = e.clientY
    showContextMenu.value = false
    nextTick(() => {
      showContextMenu.value = true
    })
  }

  async function addSongToPlaylist(playlistId: number) {
    if (!selectedSongForMenu.value) return
    try {
      await invoke('add_song_to_playlist', {
        playlistId,
        songId: selectedSongForMenu.value.id,
      })
      toast.show('Đã thêm bài hát vào danh sách phát.')
    } catch (err) {
      console.error('Failed to add song to playlist:', err)
      toast.show('Lỗi khi thêm bài hát vào danh sách phát.')
    }
  }

  async function removeSongFromPlaylist() {
    if (!selectedSongForMenu.value || props.type !== 'playlist') return
    try {
      await invoke('remove_song_from_playlist', {
        playlistId: props.id,
        songId: selectedSongForMenu.value.id,
      })
      toast.show('Đã xóa bài hát khỏi danh sách phát.')

      // Refresh list
      songs.value = await invoke<Song[]>('get_playlist_songs', {
        playlistId: props.id,
      })
    } catch (err) {
      console.error('Failed to remove song from playlist:', err)
      toast.show('Lỗi khi xóa bài hát.')
    }
  }

  function openEditMetadata() {
    isEditingMetadata.value = true
  }

  function openEditTimeline() {
    isEditingTimeline.value = true
  }

  function openEditLyrics() {
    isEditingLyrics.value = true
  }

  function onMetadataSaved(updatedSong: any) {
    const idx = songs.value.findIndex((s) => s.id === updatedSong.id)
    if (idx !== -1) {
      songs.value[idx] = {
        ...songs.value[idx],
        filename: updatedSong.filename,
        title: updatedSong.title,
        artist: updatedSong.artist,
        album_id: updatedSong.album_id,
      }
    }
  }

  function onTimelineSaved(newTimeline: string) {
    if (selectedSongForMenu.value) {
      const idx = songs.value.findIndex((s) => s.id === selectedSongForMenu.value!.id)
      if (idx !== -1) {
        songs.value[idx].timeline = newTimeline
      }
    }
  }

  function onLyricsSaved(newLyrics: string) {
    if (selectedSongForMenu.value) {
      const idx = songs.value.findIndex((s) => s.id === selectedSongForMenu.value!.id)
      if (idx !== -1) {
        songs.value[idx].lyrics = newLyrics
      }
    }
  }

  function openCreatePlaylistDialogFromMenu() {
    newPlaylistName.value = ''
    newPlaylistColor.value = '#06b6d4'
    createDialogVisible.value = true
  }

  async function handleCreatePlaylist() {
    const name = newPlaylistName.value.trim()
    if (!name) {
      toast.show('Vui lòng nhập tên danh sách phát.')
      return
    }
    isCreatingPlaylist.value = true
    try {
      const playlistId = await invoke<number>('create_playlist', {
        name,
        color: newPlaylistColor.value,
      })

      if (selectedSongForMenu.value) {
        await invoke('add_song_to_playlist', {
          playlistId,
          songId: selectedSongForMenu.value.id,
        })
        toast.show(`Đã tạo danh sách "${name}" và thêm bài hát.`)
      } else {
        toast.show(`Đã tạo danh sách phát "${name}".`)
      }

      createDialogVisible.value = false
      await loadPlaylists()
    } catch (err: any) {
      console.error('Error creating playlist from context menu:', err)
      toast.show(err.toString() || 'Lỗi khi tạo danh sách phát.')
    } finally {
      isCreatingPlaylist.value = false
    }
  }
</script>
