<template>
  <v-dialog v-model="store.isOpen" max-width="600" attach="body" position="top" transition="dialog-transition">
    <!-- Main Dialog Container with Tailwind & CSS Variables -->
    <div
      class="bg-(--cmd-bg) border border-(--cmd-border) rounded-2xl overflow-hidden shadow-2xl backdrop-blur-md text-(--cmd-text-primary) font-sans">
      <!-- Search Input Wrapper -->
      <div class="flex items-center px-5 py-3.5 border-b border-(--cmd-border) gap-3">
        <!-- Dynamic Header Icon via SvgSprite -->
        <span
          class="flex items-center justify-center text-(--cmd-text-secondary) transition-colors duration-200"
          :class="{ 'text-cmd-cli-accent': isCommandMode }">
          <svg-sprite :src="isCommandMode ? 'Command' : 'Search'" class-name="w-4 h-4" />
        </span>

        <!-- Main Input -->
        <input
          ref="inputRef"
          v-model="searchQuery"
          @keydown="handleKeyDown"
          placeholder="Gõ '/' để dùng lệnh phát nhạc, hoặc tìm bài hát, ca sĩ..."
          class="w-full bg-transparent border-none outline-none text-sm placeholder-(--cmd-input-placeholder) text-(--cmd-text-primary)" />

        <!-- Mode Indicator Badge -->
        <span
          class="text-[10px] font-mono px-1.5 py-0.5 rounded-md select-none whitespace-nowrap transition-colors duration-150 border"
          :class="
            isCommandMode
              ? 'bg-(--cmd-badge-cmd-bg) text-(--cmd-badge-cmd-text) border-(--cmd-badge-cmd-border)'
              : 'bg-(--cmd-badge-search-bg) text-(--cmd-badge-search-text) border-(--cmd-badge-search-border)'
          ">
          {{ isCommandMode ? 'CLI Mode' : 'Search' }}
        </span>
      </div>

      <!-- Scrollable Results -->
      <OverlayScrollbarsComponent class="max-h-cmd-list-max-h" defer :options="OSOptions">
        <div class="p-2">
          <!-- Empty State -->
          <div
            v-if="filteredItems.length === 0"
            class="flex flex-col items-center justify-center py-12 px-6 text-(--cmd-text-secondary) text-sm gap-3">
            <!-- <svg
							xmlns="http://www.w3.org/2000/svg"
							width="24"
							height="24"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2"
							stroke-linecap="round"
							stroke-linejoin="round"
						>
							<circle cx="12" cy="12" r="10"></circle>
							<line x1="8" y1="12" x2="16" y2="12"></line>
						</svg> -->

            <div class="text-3xl">Empty Placeholder</div>
            <span>Không tìm thấy kết quả cho "{{ searchQuery }}"</span>
          </div>

          <!-- Grouped Content -->
          <div v-else v-for="section in groupedItems" :key="section.title" class="mb-3 last:mb-0">
            <div class="text-[10px] font-bold tracking-wider text-(--cmd-text-secondary) px-3 py-1.5 select-none">
              {{ section.title }}
            </div>

            <search-item
              v-for="item in section.items"
              :key="item.id"
              :ref="(el) => setItemRef(el, item.globalIndex)"
              :item="item"
              :is-active="item.globalIndex === activeIndex"
              @select="selectItem"
              @hover="activeIndex = $event" />
          </div>
        </div>
      </OverlayScrollbarsComponent>

      <!-- Footer Help Guide -->
      <div
        class="flex items-center justify-between px-5 py-2.5 bg-(--cmd-footer-bg) border-t border-(--cmd-footer-border) text-[11px] text-(--cmd-text-secondary)">
        <div class="flex items-center gap-1.5">
          <kbd class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25">
            ↑↓
          </kbd>
          Di chuyển
          <span class="opacity-50">•</span>
          <span v-if="isCommandMode" class="flex items-center gap-1.5">
            <kbd class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25">
              Tab
            </kbd>
            Chọn lệnh
            <span class="opacity-50">•</span>
            <kbd class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25">
              ↵ Enter
            </kbd>
            Chạy lệnh
          </span>
          <span v-else class="flex items-center gap-1.5">
            <kbd class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25">
              ↵ Enter
            </kbd>
            Chọn
          </span>
        </div>
        <div class="flex items-center gap-1.5">
          <kbd class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25">
            Esc
          </kbd>
          Đóng
        </div>
      </div>
    </div>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick } from 'vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { useCommandPaletteStore, useAudioPlayer } from '@groovex/store'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import { invoke } from '@tauri-apps/api/core'
  import SearchItem from './search-item.vue'
  import type { PartialOptions } from 'overlayscrollbars'
  import { CmdQuickActions, KeyboardShortcuts } from './command.const.ts'

  const store = useCommandPaletteStore()
  const player = useAudioPlayer()
  const OSOptions: PartialOptions = { scrollbars: { autoHide: 'scroll' } }

  const searchQuery = ref('')
  const activeIndex = ref(0)
  const inputRef = ref<HTMLInputElement | null>(null)
  const itemRefs = ref<Record<number, HTMLElement>>({})
  const searchResults = ref<any[]>([])
  let searchTimeout: number

  // Mode Detector (Starts with /)
  const isCommandMode = computed(() => {
    return searchQuery.value.trim().startsWith('/')
  })

  // List of Built-in CLI commands
  const COMMANDS = KeyboardShortcuts

  // Default Quick Actions when empty input
  const QUICK_ACTIONS = CmdQuickActions

  // Watch searchQuery to perform debounced DB queries
  watch(searchQuery, (newQuery) => {
    if (searchTimeout) clearTimeout(searchTimeout)

    const query = newQuery.trim()
    if (!query || query.startsWith('/')) {
      searchResults.value = []
      return
    }

    searchTimeout = window.setTimeout(async () => {
      try {
        const dbSongs = await invoke<any[]>('search_songs', { query })
        searchResults.value = dbSongs.map((song) => ({
          id: `song-${song.id}`,
          type: 'song',
          title: song.title || song.filename,
          description: `Bài hát • ${song.artist || 'Chưa rõ nghệ sĩ'}${song.album_name ? ` • ${song.album_name}` : ''}`,
          rawSong: song,
        }))
      } catch (err) {
        console.error('Lỗi tìm kiếm bài hát:', err)
      }
    }, 250)
  })

  // Filter based on input value
  const filteredItems = computed(() => {
    const rawQuery = searchQuery.value.trim()
    const query = rawQuery.toLowerCase()

    if (rawQuery.startsWith('/')) {
      if (rawQuery === '/') return COMMANDS
      return COMMANDS.filter((c) => c.commandName.toLowerCase().includes(query))
    } else {
      if (!query) return QUICK_ACTIONS
      return searchResults.value
    }
  })

  // Group items by type for nice visual sectioning
  const groupedItems = computed(() => {
    const items = filteredItems.value
    const groups: Record<string, any[]> = {}

    items.forEach((item, index) => {
      // Create a shallow copy to store globalIndex without mutating mock data
      const itemWithIndex = { ...item, globalIndex: index }

      let groupTitle = 'Kết quả'
      if (item.type === 'command') {
        groupTitle = 'Lệnh hệ thống (CLI Commands)'
      } else if (item.type === 'song') {
        groupTitle = 'Bài hát'
      } else if (item.type === 'artist') {
        groupTitle = 'Nghệ sĩ'
      } else if (item.type === 'album') {
        groupTitle = 'Album'
      } else if (item.type === 'action') {
        groupTitle = 'Hành động nhanh (Quick Actions)'
      }

      if (!groups[groupTitle]) {
        groups[groupTitle] = []
      }
      groups[groupTitle].push(itemWithIndex)
    })

    return Object.keys(groups).map((title) => ({
      title,
      items: groups[title],
    }))
  })

  // Reset indices and auto focus input on open
  watch(
    () => store.isOpen,
    async (isOpen) => {
      if (isOpen) {
        searchQuery.value = ''
        searchResults.value = []
        activeIndex.value = 0
        itemRefs.value = {}
        await nextTick()
        inputRef.value?.focus()
      }
    },
  )

  // Auto bounds correction for navigation index
  watch(
    () => filteredItems.value.length,
    (len) => {
      if (activeIndex.value >= len) {
        activeIndex.value = Math.max(0, len - 1)
      }
    },
  )

  // Ref assignments
  function setItemRef(el: any, index: number) {
    if (el) {
      itemRefs.value[index] = el.$el || el
    }
  }

  // Keyboard navigation, autocomplete & execution
  function handleKeyDown(e: KeyboardEvent) {
    const len = filteredItems.value.length

    if (e.key === 'Tab') {
      if (len > 0) {
        e.preventDefault() // Prevent focus from changing
        const selected = filteredItems.value[activeIndex.value]
        if (selected) {
          if (selected.type === 'command') {
            autocompleteCommand(selected)
          } else {
            selectItem(selected)
          }
        }
      }
    } else if (e.key === 'ArrowDown') {
      if (len === 0) return
      e.preventDefault()
      activeIndex.value = (activeIndex.value + 1) % len
      scrollToActiveItem()
    } else if (e.key === 'ArrowUp') {
      if (len === 0) return
      e.preventDefault()
      activeIndex.value = (activeIndex.value - 1 + len) % len
      scrollToActiveItem()
    } else if (e.key === 'Enter') {
      e.preventDefault()
      if (isCommandMode.value) {
        // Run typed command text
        runCommand(searchQuery.value)
      } else {
        if (len > 0) {
          const selected = filteredItems.value[activeIndex.value]
          if (selected) {
            selectItem(selected)
          }
        }
      }
    }
  }

  // Handle selection (click or select via non-command action)
  function selectItem(item: any) {
    console.log(`selected item`, item)

    if (item.type === 'command') {
      autocompleteCommand(item)
    } else if (item.type === 'action') {
      console.log(`Triggered Quick Action: ${item.title}`)
      if (item.id === 'qa-1') {
        // Play random from library if we can, or play the active list shuffled
        if (player.playlist && player.playlist.length > 0) {
          player.isShuffle = true
          player.playSong(player.playlist[Math.floor(Math.random() * player.playlist.length)])
        }
      }
      // store.close();  // dont enable this
    } else if (item.type === 'song') {
      const song = item.rawSong
      if (player.currentSong?.id === song.id) {
        player.togglePlay()
      } else {
        player.playSong(song, [song])
      }
      // store.close();  // dont enable this
    } else {
      console.log(`Selected Search Result: ${item.title} (${item.type})`)
    }
  }

  // Autocomplete command into the input field
  function autocompleteCommand(item: any) {
    const needsArgs = ['/play', '/queue', '/volume', '/theme'].includes(item.commandName)
    if (needsArgs) {
      searchQuery.value = item.commandName + ' '
    } else {
      searchQuery.value = item.commandName
    }
    activeIndex.value = 0
    nextTick(() => {
      inputRef.value?.focus()
    })
  }

  // Run the command specified in the search query
  function runCommand(queryText: string) {
    const trimmed = queryText.trim()
    if (!trimmed.startsWith('/')) return

    const spaceIndex = trimmed.indexOf(' ')
    const cmd = spaceIndex === -1 ? trimmed : trimmed.substring(0, spaceIndex)
    const args = spaceIndex === -1 ? '' : trimmed.substring(spaceIndex + 1).trim()

    console.log(`[CLI Run] Running command: "${cmd}" with args: "${args}"`)

    if (cmd === '/play') {
      if (args) {
        invoke<any[]>('search_songs', { query: args })
          .then((songs) => {
            if (songs && songs.length > 0) {
              player.playSong(songs[0], songs)
            }
          })
          .catch((err) => console.error(err))
      } else {
        player.setPlaying(true)
      }
    } else if (cmd === '/pause') {
      player.setPlaying(false)
    } else if (cmd === '/next') {
      player.nextTrack()
    } else if (cmd === '/prev') {
      player.prevTrack()
    } else if (cmd === '/playall') {
      if (player.playlist && player.playlist.length > 0) {
        player.playSong(player.playlist[0])
      }
    } else if (cmd === '/volume') {
      const vol = parseInt(args)
      if (!isNaN(vol)) {
        player.volume = Math.max(0, Math.min(100, vol))
      }
    } else {
      // For other commands (like theme/help) we can log or show default alert
      alert(`[CLI Run] Đang thực thi lệnh: ${cmd}${args ? ` với đối số: "${args}"` : ''}`)
    }

    // Close the command palette
    store.close()
  }

  // Scroll active item smoothly into view
  function scrollToActiveItem() {
    nextTick(() => {
      const activeEl = itemRefs.value[activeIndex.value]
      if (activeEl) {
        activeEl.scrollIntoView({ block: 'nearest' })
      }
    })
  }
</script>
