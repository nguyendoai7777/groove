<template>
  <div class="h-full">
    <!-- View Switcher -->
    <transition name="fade" mode="out-in">
      <!-- Grid Category View -->
      <div v-if="currentView === 'grid'" class="p-6">
        <!-- Header -->
        <div class="flex pb-4 justify-between items-center mb-6">
          <h1 class="text-3xl font-light text-theme-text">{{ mode === 'playlists' ? 'Playlists' : 'My Music' }}</h1>
          <div class="flex gap-2">
            <icon-btn title="Settings" :size="{ icon: 'sm' }" src="Settings" />
          </div>
        </div>

        <!-- Create Playlist Banner (Playlists Mode) -->
        <div
          v-if="mode === 'playlists'"
          @click="openCreatePlaylistDialog"
          class="flex items-start gap-3.5 p-4 bg-theme-bg-card hover:bg-theme-bg-card-hover border border-theme-border/80 hover:border-theme-border-hover/80 rounded-lg cursor-pointer transition-all duration-200 mb-8 max-w-xl select-none">
          <div class="shrink-0 mt-0.5">
            <svg-sprite src="Album" class="w-5 h-5 text-theme-accent-light" />
          </div>
          <div class="flex flex-col text-left">
            <span class="text-[13px] font-semibold text-theme-text-secondary">Create a New Playlist</span>
            <span class="text-xs text-theme-accent-light hover:text-theme-accent hover:underline mt-1 font-medium">
              Group your favorite tracks together
            </span>
          </div>
        </div>

        <!-- Import Banner (My Music Mode) -->
        <div
          v-else
          @click="handleImportClick"
          class="flex items-start gap-3.5 p-4 bg-theme-bg-card hover:bg-theme-bg-card-hover border border-theme-border/80 hover:border-theme-border-hover/80 rounded-lg cursor-pointer transition-all duration-200 mb-8 max-w-xl select-none">
          <div class="shrink-0 mt-0.5">
            <svg-sprite src="Folder" class="w-5 h-5 text-theme-text-muted" />
          </div>
          <div class="flex flex-col text-left">
            <span class="text-[13px] font-semibold text-theme-text-secondary">Not finding everything?</span>
            <span class="text-xs text-theme-accent-light hover:text-theme-accent hover:underline mt-1 font-medium">
              Show us where to look for music
            </span>
          </div>
        </div>

        <!-- Category View for My Music Mode -->
        <template v-if="mode === 'my-music'">
          <v-tabs v-model="activeTab" color="cyan-lighten-1" class="mb-6 border-b border-theme-border">
            <v-tab value="albums" class="text-theme-text-secondary font-semibold">Albums</v-tab>
            <v-tab value="folders" class="text-theme-text-secondary font-semibold">Folders</v-tab>
          </v-tabs>

          <v-tabs-window v-model="activeTab">
            <v-tabs-window-item value="albums">
              <div v-if="albums.length > 0 || (isImporting && activeTab === 'albums')">
                <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
                  <music-card
                    v-for="item in albums"
                    :key="'album-' + item.id"
                    type="album"
                    :title="item.title"
                    :subtitle="item.subtitle"
                    :thumbnail="item.thumbnail"
                    :songs-count="item.songsCount"
                    :accent-color="item.accentColor"
                    @click="openCategoryDetails(item)"
                    @play="handlePlay"
                    @settings="handleSettings(item)"
                    @delete="handleDeleteCategory(item)" />
                  <!-- Loading Skeleton Card -->
                  <div
                    v-if="isImporting && activeTab === 'albums'"
                    class="music-card flex flex-col p-3 rounded-xl border-2 border-theme-border/20 bg-theme-bg-card/30 select-none">
                    <div
                      class="relative w-full aspect-square rounded-lg overflow-hidden bg-theme-bg-placeholder flex items-center justify-center shadow-md">
                      <v-progress-circular indeterminate color="cyan" size="40" width="4" />
                    </div>
                    <div class="mt-3 flex flex-col min-w-0">
                      <span class="text-sm font-semibold truncate text-theme-text-secondary">Importing...</span>
                      <span class="text-xs text-theme-text-disabled mt-1 truncate">Scanning directory</span>
                    </div>
                  </div>
                </div>
              </div>
              <div class="flex flex-col items-center justify-center py-20 text-center select-none" v-else>
                <svg-sprite src="Album" class="w-16 h-16 text-theme-text-disabled/60 mb-4" />
                <h3 class="text-lg font-semibold text-theme-text-muted">No albums found</h3>
                <p class="text-xs text-theme-text-disabled mt-2 max-w-sm">
                  Import folders containing audio files with album metadata to see them here.
                </p>
              </div>
            </v-tabs-window-item>

            <!-- Folders Tab -->
            <v-tabs-window-item value="folders">
              <div v-if="folders.length > 0 || (isImporting && activeTab === 'folders')">
                <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
                  <music-card
                    v-for="item in folders"
                    :key="'folder-' + item.id"
                    type="folder"
                    :title="item.title"
                    :subtitle="item.subtitle"
                    :thumbnail="item.thumbnail"
                    :songs-count="item.songsCount"
                    :accent-color="item.accentColor"
                    @click="openCategoryDetails(item)"
                    @play="handlePlay"
                    @settings="handleSettings(item)"
                    @delete="handleDeleteCategory(item)" />
                  <!-- Loading Skeleton Card -->
                  <div
                    v-if="isImporting && activeTab === 'folders'"
                    class="music-card flex flex-col p-3 rounded-xl border-2 border-theme-border/20 bg-theme-bg-card/30 select-none">
                    <div
                      class="relative w-full aspect-square rounded-lg overflow-hidden bg-theme-bg-placeholder flex items-center justify-center shadow-md">
                      <v-progress-circular indeterminate color="cyan" size="40" width="4" />
                    </div>
                    <div class="mt-3 flex flex-col min-w-0">
                      <span class="text-sm font-semibold truncate text-theme-text-secondary">Importing...</span>
                      <span class="text-xs text-theme-text-disabled mt-1 truncate">Scanning directory</span>
                    </div>
                  </div>
                </div>
              </div>
              <div class="flex flex-col items-center justify-center py-20 text-center select-none" v-else>
                <svg-sprite src="Folder" class="w-16 h-16 text-theme-text-disabled/60 mb-4" />
                <h3 class="text-lg font-semibold text-theme-text-muted">No folders imported</h3>
                <p class="text-xs text-theme-text-disabled mt-2 max-w-sm">Click the banner above to import a folder containing music.</p>
              </div>
            </v-tabs-window-item>
          </v-tabs-window>
        </template>

        <!-- Playlist Grid View (Playlists Mode) -->
        <template v-else>
          <div v-if="playlists.length > 0">
            <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-4 lg:grid-cols-5 xl:grid-cols-6 gap-6">
              <music-card
                v-for="item in playlists"
                :key="'playlist-' + item.id"
                type="playlist"
                :title="item.title"
                :songs-count="item.songsCount"
                :accent-color="item.accentColor"
                @click="openCategoryDetails(item)"
                @play="handlePlay"
                @delete="handleDeleteCategory(item)" />
            </div>
          </div>
          <div class="flex flex-col items-center justify-center py-20 text-center select-none" v-else>
            <svg-sprite src="Album" class="w-16 h-16 text-theme-text-disabled/60 mb-4" />
            <h3 class="text-lg font-semibold text-theme-text-muted">No playlists found</h3>
            <p class="text-xs text-theme-text-disabled mt-2 max-w-sm">Create a new playlist to group your songs together.</p>
          </div>
        </template>
      </div>

      <!-- Detail Song List View -->
      <music-list
        v-else-if="currentView === 'list' && selectedCategory"
        :type="selectedCategory.type"
        :id="selectedCategory.id"
        :title="selectedCategory.title"
        :subtitle="selectedCategory.subtitle"
        :thumbnail="selectedCategory.thumbnail"
        :accent-color="selectedCategory.accentColor"
        :songs-count="selectedCategory.songsCount"
        @back="router.push(mode === 'playlists' ? '/playlists' : '/my-music')" />
    </transition>

    <!-- Create Playlist Dialog -->
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
  import { ref, computed, onMounted, watch } from 'vue';
  import { useRoute, useRouter } from 'vue-router';
  import { invoke } from '@tauri-apps/api/core';
  import IconBtn from '@groovex/ui/button/icon-btn.vue';
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue';
  import { MusicCard } from '@groovex/ui/music-card';
  import MusicList from '@groovex/ui/music-list/music-list.vue';
  import { useAudioPlayer } from '@groovex/store';
  import type { Song } from '@groovex/types';
  import { useToast } from '@groovex/composables';

  interface MusicItemCard {
    id: number;
    type: 'album' | 'folder' | 'playlist';
    title: string;
    subtitle: string;
    thumbnail?: string;
    songsCount?: number;
    accentColor?: string;
  }

  const props = withDefaults(
    defineProps<{
      mode?: 'my-music' | 'playlists';
    }>(),
    {
      mode: 'my-music',
    },
  );

  const route = useRoute();
  const router = useRouter();

  const currentView = computed(() => (route.query.view === 'list' ? 'list' : 'grid'));
  const selectedCategory = computed(() => {
    if (route.query.view !== 'list') return null;
    return {
      id: Number(route.query.id),
      type: route.query.type as 'album' | 'folder' | 'playlist',
      title: route.query.title as string,
      subtitle: route.query.subtitle as string,
      thumbnail: (route.query.thumbnail as string) || undefined,
      accentColor: (route.query.accentColor as string) || undefined,
      songsCount: route.query.songsCount ? Number(route.query.songsCount) : undefined,
    };
  });

  const isImporting = ref(false);
  const musicItems = ref<MusicItemCard[]>([]);
  const playlists = ref<MusicItemCard[]>([]);

  const activeTab = ref<'albums' | 'folders'>('albums');
  const albums = computed(() => musicItems.value.filter((item) => item.type === 'album'));
  const folders = computed(() => musicItems.value.filter((item) => item.type === 'folder'));

  // Playlist creation states
  const createDialogVisible = ref(false);
  const isCreatingPlaylist = ref(false);
  const newPlaylistName = ref('');
  const newPlaylistColor = ref('#06b6d4');

  // Load imported items from SQLite DB
  async function loadCategories() {
    try {
      if (props.mode === 'playlists') {
        const res = await invoke<any[]>('get_playlists');
        playlists.value = res.map((p) => ({
          id: p.id,
          type: 'playlist' as const,
          title: p.name,
          subtitle: '',
          songsCount: p.songs_count,
          accentColor: p.color || '#06b6d4',
        }));
      } else {
        const res = await invoke<{ folders: any[]; albums: any[] }>('get_music_categories');

        const folderCards = res.folders.map((f) => ({
          id: f.id,
          type: 'folder' as const,
          title: f.name,
          subtitle: f.path,
          thumbnail: f.thumbnail,
          songsCount: f.songs_count,
          accentColor: f.accent_color || '#0ea5e9',
        }));

        const albumCards = res.albums.map((a) => ({
          id: a.id,
          type: 'album' as const,
          title: a.name,
          subtitle: a.artist || 'Unknown Artist',
          thumbnail: a.thumbnail,
          songsCount: a.songs_count,
          accentColor: a.accent_color || '#4b5563',
        }));

        musicItems.value = [...folderCards, ...albumCards];
      }
    } catch (err) {
      console.error('Error loading categories:', err);
    }
  }

  onMounted(() => {
    loadCategories();
  });

  // Watch for mode changes to refresh categories
  watch(
    () => props.mode,
    () => {
      loadCategories();
    },
  );

  const toast = useToast();

  // Trigger folder picker and scanning
  async function handleImportClick() {
    isImporting.value = true;
    try {
      const res = await invoke<{
        folders: any[];
        albums: any[];
        added_count: number;
        removed_count: number;
      }>('import_music_folder');

      const folderCards = res.folders.map((f) => ({
        id: f.id,
        type: 'folder' as const,
        title: f.name,
        subtitle: f.path,
        thumbnail: f.thumbnail,
        songsCount: f.songs_count,
        accentColor: f.accent_color || '#0ea5e9',
      }));

      const albumCards = res.albums.map((a) => ({
        id: a.id,
        type: 'album' as const,
        title: a.name,
        subtitle: a.artist || 'Unknown Artist',
        thumbnail: a.thumbnail,
        songsCount: a.songs_count,
        accentColor: a.accent_color || '#4b5563',
      }));

      musicItems.value = [...folderCards, ...albumCards];

      // Show toast notification
      if (res.added_count > 0 && res.removed_count > 0) {
        toast.show(`Có ${res.added_count} bài vừa được thêm và ${res.removed_count} bài vừa được xóa.`);
      } else if (res.added_count > 0) {
        toast.show(`Có ${res.added_count} bài vừa được thêm.`);
      } else if (res.removed_count > 0) {
        toast.show(`Có ${res.removed_count} bài vừa được xóa.`);
      } else {
        toast.show('Không có bài hát nào mới được thêm hoặc xóa.');
      }
    } catch (err) {
      console.error('Error scanning folder:', err);
      toast.show('Đã xảy ra lỗi khi quét thư mục.');
    } finally {
      isImporting.value = false;
    }
  }

  // Opens category song listing via route parameters for back-button history integration
  function openCategoryDetails(item: MusicItemCard) {
    router.push({
      path: props.mode === 'playlists' ? '/playlists' : '/my-music',
      query: {
        view: 'list',
        type: item.type,
        id: item.id.toString(),
        title: item.title,
        subtitle: item.subtitle,
        thumbnail: item.thumbnail || '',
        accentColor: item.accentColor || '',
        songsCount: item.songsCount ? item.songsCount.toString() : '',
      },
    });
  }

  // Custom thumbnail setting picker
  async function handleSettings(item: MusicItemCard) {
    if (item.type === 'playlist') return;
    try {
      const newThumbnailBase64 = await invoke<string>('update_music_thumbnail', {
        categoryType: item.type,
        categoryId: item.id,
      });

      // Extract accent color using HTML canvas on the fly
      const accentColor = await getAverageColor(newThumbnailBase64);

      // Save accent color in DB
      await invoke('update_category_accent_color', {
        categoryType: item.type,
        categoryId: item.id,
        accentColor,
      });

      // Reload list to refresh accent Color and thumbnail
      await loadCategories();
    } catch (err) {
      if (err !== 'Cancelled') {
        console.error('Error updating custom cover art:', err);
      }
    }
  }

  async function handleDeleteCategory(item: MusicItemCard) {
    try {
      if (item.type === 'playlist') {
        await invoke('delete_playlist', { id: item.id });
        toast.show(`Đã xóa danh sách phát "${item.title}".`);
        await loadCategories();
      } else {
        const res = await invoke<{ folders: any[]; albums: any[] }>('delete_category', {
          categoryType: item.type,
          categoryId: item.id,
        });

        const folderCards = res.folders.map((f) => ({
          id: f.id,
          type: 'folder' as const,
          title: f.name,
          subtitle: f.path,
          thumbnail: f.thumbnail,
          songsCount: f.songs_count,
          accentColor: f.accent_color || '#0ea5e9',
        }));

        const albumCards = res.albums.map((a) => ({
          id: a.id,
          type: 'album' as const,
          title: a.name,
          subtitle: a.artist || 'Unknown Artist',
          thumbnail: a.thumbnail,
          songsCount: a.songs_count,
          accentColor: a.accent_color || '#4b5563',
        }));

        musicItems.value = [...folderCards, ...albumCards];
      }
    } catch (err) {
      console.error('Error deleting category:', err);
    }
  }

  async function handlePlay(item: { type: 'album' | 'folder' | 'playlist'; title: string }) {
    const listToSearch = props.mode === 'playlists' ? playlists.value : musicItems.value;
    const matched = listToSearch.find((i) => i.type === item.type && i.title === item.title);
    if (matched) {
      try {
        let categorySongs: Song[] = [];
        if (matched.type === 'playlist') {
          categorySongs = await invoke<Song[]>('get_playlist_songs', {
            playlistId: matched.id,
          });
        } else {
          categorySongs = await invoke<Song[]>('get_category_songs', {
            categoryType: matched.type,
            categoryId: matched.id,
          });
        }
        if (categorySongs.length > 0) {
          const playlist = categorySongs.map((s) => ({
            ...s,
            thumbnail: matched.thumbnail,
          }));
          const player = useAudioPlayer();
          player.playSong(playlist[0], playlist);
        } else {
          toast.show('Danh sách phát này chưa có bài hát nào.');
        }
      } catch (err) {
        console.error('Error playing category:', err);
      }
    }
  }

  // Helper function to extract average dominant color from base64 string
  function getAverageColor(base64Image: string): Promise<string> {
    return new Promise((resolve) => {
      const img = new Image();
      img.onload = () => {
        const canvas = document.createElement('canvas');
        canvas.width = 1;
        canvas.height = 1;
        const ctx = canvas.getContext('2d');
        if (ctx) {
          ctx.drawImage(img, 0, 0, 1, 1);
          const pixel = ctx.getImageData(0, 0, 1, 1).data;
          // Return format rgb(r, g, b)
          resolve(`rgb(${pixel[0]}, ${pixel[1]}, ${pixel[2]})`);
        } else {
          resolve('#06b6d4');
        }
      };
      img.onerror = () => resolve('#06b6d4');
      img.src = base64Image;
    });
  }

  // Playlist creation dialog triggers
  function openCreatePlaylistDialog() {
    newPlaylistName.value = '';
    newPlaylistColor.value = '#06b6d4';
    createDialogVisible.value = true;
  }

  async function handleCreatePlaylist() {
    const name = newPlaylistName.value.trim();
    if (!name) {
      toast.show('Vui lòng nhập tên danh sách phát.');
      return;
    }
    isCreatingPlaylist.value = true;
    try {
      await invoke('create_playlist', {
        name,
        color: newPlaylistColor.value,
      });
      toast.show(`Đã tạo danh sách phát "${name}".`);
      createDialogVisible.value = false;
      await loadCategories();
    } catch (err: any) {
      console.error('Error creating playlist:', err);
      toast.show(err.toString() || 'Đã xảy ra lỗi khi tạo danh sách phát.');
    } finally {
      isCreatingPlaylist.value = false;
    }
  }
</script>

<style>
  .fade-enter-active,
  .fade-leave-active {
    transition: opacity 0.2s ease;
  }
  .fade-enter-from,
  .fade-leave-to {
    opacity: 0;
  }
</style>
