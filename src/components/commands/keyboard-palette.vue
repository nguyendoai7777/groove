<template>
  <v-dialog
    v-model="store.isOpen"
    max-width="600"
    attach="body"
    position="top"
    transition="dialog-transition"
  >
    <!-- Main Dialog Container with Tailwind & CSS Variables -->
    <div
      class="bg-(--cmd-bg) border border-(--cmd-border) rounded-2xl overflow-hidden shadow-2xl backdrop-blur-md text-(--cmd-text-primary) font-sans"
    >
      <!-- Search Input Wrapper -->
      <div class="flex items-center px-5 py-3.5 border-b border-(--cmd-border) gap-3">
        <!-- Dynamic Header Icon via SvgSprite -->
        <span
          class="flex items-center justify-center text-(--cmd-text-secondary) transition-colors duration-200"
          :class="{ 'text-indigo-400': isCommandMode }"
        >
          <svg-sprite :src="isCommandMode ? 'Command' : 'Search'" class-name="w-4 h-4" />
        </span>

        <!-- Main Input -->
        <input
          ref="inputRef"
          v-model="searchQuery"
          @keydown="handleKeyDown"
          placeholder="Gõ '/' để dùng lệnh phát nhạc, hoặc tìm bài hát, ca sĩ..."
          class="w-full bg-transparent border-none outline-none text-sm placeholder-(--cmd-input-placeholder) text-(--cmd-text-primary)"
        />

        <!-- Mode Indicator Badge -->
        <span
          class="text-[10px] font-mono px-1.5 py-0.5 rounded-md select-none whitespace-nowrap transition-colors duration-150 border"
          :class="
            isCommandMode
              ? 'bg-(--cmd-badge-cmd-bg) text-(--cmd-badge-cmd-text) border-(--cmd-badge-cmd-border)'
              : 'bg-(--cmd-badge-search-bg) text-(--cmd-badge-search-text) border-(--cmd-badge-search-border)'
          "
        >
          {{ isCommandMode ? 'CLI Mode' : 'Search' }}
        </span>
      </div>

      <!-- Scrollable Results -->
      <OverlayScrollbarsComponent class="max-h-[380px]" defer :options="OSOptions">
        <div class="p-2">
          <!-- Empty State -->
          <div
            v-if="filteredItems.length === 0"
            class="flex flex-col items-center justify-center py-12 px-6 text-(--cmd-text-secondary) text-sm gap-3"
          >
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

            <div class="text-3xl">Empty Placegolder</div>
            <span>Không tìm thấy kết quả cho "{{ searchQuery }}"</span>
          </div>

          <!-- Grouped Content -->
          <div v-else v-for="section in groupedItems" :key="section.title" class="mb-3 last:mb-0">
            <div
              class="text-[10px] font-bold uppercase tracking-wider text-(--cmd-text-secondary) px-3 py-1.5 select-none"
            >
              {{ section.title }}
            </div>

            <search-item
              v-for="item in section.items"
              :key="item.id"
              :ref="(el) => setItemRef(el, item.globalIndex)"
              :item="item"
              :is-active="item.globalIndex === activeIndex"
              @select="selectItem"
              @hover="activeIndex = $event"
            />
          </div>
        </div>
      </OverlayScrollbarsComponent>

      <!-- Footer Help Guide -->
      <div
        class="flex items-center justify-between px-5 py-2.5 bg-(--cmd-footer-bg) border-t border-(--cmd-footer-border) text-[11px] text-(--cmd-text-secondary)"
      >
        <div class="flex items-center gap-1.5">
          <span
            ><kbd
              class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25"
              >↑↓</kbd
            >
            Di chuyển</span
          >
          <span class="opacity-50">•</span>
          <span v-if="isCommandMode" class="flex items-center gap-1.5">
            <kbd
              class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25"
              >Tab</kbd
            >
            Chọn lệnh
            <span class="opacity-50">•</span>
            <kbd
              class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25"
              >↵ Enter</kbd
            >
            Chạy lệnh
          </span>
          <span v-else class="flex items-center gap-1.5">
            <kbd
              class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25"
              >↵ Enter</kbd
            >
            Chọn
          </span>
        </div>
        <div>
          <span
            ><kbd
              class="font-mono text-[9px] bg-(--cmd-kbd-bg) text-(--cmd-kbd-text) border border-(--cmd-kbd-border) rounded px-1 py-0.25"
              >Esc</kbd
            >
            Đóng</span
          >
        </div>
      </div>
    </div>
  </v-dialog>
</template>

<script setup lang="ts">
  import { ref, computed, watch, nextTick } from 'vue'
  import { OverlayScrollbarsComponent } from 'overlayscrollbars-vue'
  import { useCommandPaletteStore } from '@groovex/store'
  import SvgSprite from '@groovex/ui/svg-sprite/svg-sprite.vue'
  import SearchItem from './search-item.vue'
  import type { PartialOptions } from 'overlayscrollbars'

  const store = useCommandPaletteStore()
  const OSOptions: PartialOptions = { scrollbars: { autoHide: 'scroll' } }

  const searchQuery = ref('')
  const activeIndex = ref(0)
  const inputRef = ref<HTMLInputElement | null>(null)
  const itemRefs = ref<Record<number, HTMLElement>>({})

  // Mode Detector (Starts with /)
  const isCommandMode = computed(() => {
    return searchQuery.value.trim().startsWith('/')
  })

  // List of Built-in CLI commands
  const COMMANDS = [
    {
      id: 'c-play',
      type: 'command',
      title: '/play <tên bài>',
      description: 'Phát ngay một bài hát theo tên',
      shortcut: 'Tab',
      commandName: '/play',
    },
    {
      id: 'c-playall',
      type: 'command',
      title: '/playall',
      description: 'Phát toàn bộ danh sách bài hát',
      shortcut: 'Tab',
      commandName: '/playall',
    },
    {
      id: 'c-queue',
      type: 'command',
      title: '/queue <tên bài>',
      description: 'Thêm bài hát vào hàng chờ phát',
      shortcut: 'Tab',
      commandName: '/queue',
    },
    {
      id: 'c-pause',
      type: 'command',
      title: '/pause',
      description: 'Tạm dừng bài hát đang phát',
      shortcut: 'Tab',
      commandName: '/pause',
    },
    {
      id: 'c-next',
      type: 'command',
      title: '/next',
      description: 'Chuyển sang bài hát tiếp theo',
      shortcut: 'Tab',
      commandName: '/next',
    },
    {
      id: 'c-volume',
      type: 'command',
      title: '/volume <0-100>',
      description: 'Điều chỉnh mức âm lượng phát',
      shortcut: 'Tab',
      commandName: '/volume',
    },
    {
      id: 'c-theme',
      type: 'command',
      title: '/theme <dark|light>',
      description: 'Thay đổi giao diện sáng hoặc tối',
      shortcut: 'Tab',
      commandName: '/theme',
    },
    {
      id: 'c-help',
      type: 'command',
      title: '/help',
      description: 'Hiển thị danh sách tất cả câu lệnh',
      shortcut: 'Tab',
      commandName: '/help',
    },
  ]

  // Default Quick Actions when empty input
  const QUICK_ACTIONS = [
    {
      id: 'qa-1',
      type: 'action',
      title: 'Phát ngẫu nhiên thư viện',
      description: 'Chức năng • Trộn bài hát ngẫu nhiên',
      shortcut: '⌥P',
    },
    {
      id: 'qa-2',
      type: 'action',
      title: 'Đi tới Thư viện nhạc',
      description: 'Điều hướng • Mở trang My Music',
      shortcut: 'G M',
    },
    {
      id: 'qa-3',
      type: 'action',
      title: 'Đi tới Đang phát',
      description: 'Điều hướng • Mở trang Now Playing',
      shortcut: 'G P',
    },
  ]

  // Mock search results (Songs, Artists, Albums)
  const MOCK_ITEMS = [
    {
      id: 's-1',
      type: 'song',
      title: 'Shape of You',
      description: 'Bài hát • Ed Sheeran • Divide',
      shortcut: '⌘S',
    },
    {
      id: 's-2',
      type: 'song',
      title: 'Blinding Lights',
      description: 'Bài hát • The Weeknd • After Hours',
      shortcut: '⌘S',
    },
    {
      id: 's-3',
      type: 'song',
      title: 'Bohemian Rhapsody',
      description: 'Bài hát • Queen • A Night at the Opera',
      shortcut: '⌘S',
    },
    {
      id: 's-4',
      type: 'song',
      title: 'Stay',
      description: 'Bài hát • The Kid LAROI & Justin Bieber • F*CK LOVE 3',
      shortcut: '⌘S',
    },
    {
      id: 's-5',
      type: 'song',
      title: 'Chúng Ta Của Tương Lai',
      description: 'Bài hát • Sơn Tùng M-TP • Single',
      shortcut: '⌘S',
    },
    {
      id: 's-6',
      type: 'song',
      title: 'Bad Guy',
      description: 'Bài hát • Billie Eilish • When We All Fall Asleep',
      shortcut: '⌘S',
    },
    {
      id: 'a-1',
      type: 'artist',
      title: 'Taylor Swift',
      description: 'Nghệ sĩ • 110M người nghe hàng tháng',
      shortcut: '⌘A',
    },
    {
      id: 'a-2',
      type: 'artist',
      title: 'The Weeknd',
      description: 'Nghệ sĩ • 115M người nghe hàng tháng',
      shortcut: '⌘A',
    },
    {
      id: 'a-3',
      type: 'artist',
      title: 'Sơn Tùng M-TP',
      description: 'Nghệ sĩ • 3M người nghe hàng tháng',
      shortcut: '⌘A',
    },
    {
      id: 'a-4',
      type: 'artist',
      title: 'Billie Eilish',
      description: 'Nghệ sĩ • 98M người nghe hàng tháng',
      shortcut: '⌘A',
    },
    {
      id: 'al-1',
      type: 'album',
      title: 'Random Access Memories',
      description: 'Album • Daft Punk • 13 bài hát',
      shortcut: '⌘B',
    },
    {
      id: 'al-2',
      type: 'album',
      title: 'Starboy',
      description: 'Album • The Weeknd • 18 bài hát',
      shortcut: '⌘B',
    },
    {
      id: 'al-3',
      type: 'album',
      title: 'Lover',
      description: 'Album • Taylor Swift • 18 bài hát',
      shortcut: '⌘B',
    },
  ]

  // Filter based on input value
  const filteredItems = computed(() => {
    const rawQuery = searchQuery.value.trim()
    const query = rawQuery.toLowerCase()

    if (rawQuery.startsWith('/')) {
      if (rawQuery === '/') return COMMANDS
      return COMMANDS.filter((c) => c.commandName.toLowerCase().includes(query))
    } else {
      if (!query) return QUICK_ACTIONS
      return MOCK_ITEMS.filter(
        (item) =>
          item.title.toLowerCase().includes(query) ||
          item.description.toLowerCase().includes(query),
      )
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
      store.close()
    } else {
      console.log(`Selected Search Result: ${item.title} (${item.type})`)
      // Implement DB or Tauri search interaction later
      store.close()
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
    alert(`[CLI Run] Đang thực thi lệnh: ${cmd}${args ? ` với đối số: "${args}"` : ''}`)

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
