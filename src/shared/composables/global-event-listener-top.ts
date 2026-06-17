// composables/useKeyboardShortcuts.ts
import { onMounted, onUnmounted, watch } from 'vue'
import { useCommandPaletteStore } from '@groovex/store'
import { useAudioPlayer } from '@groovex/state'

interface KeyboardCommand {
  key: string
  ctrl?: boolean
  shift?: boolean
  alt?: boolean
  action: () => void
}

export function useKeyboardShortcuts() {
  const commandPalette = useCommandPaletteStore()
  const player = useAudioPlayer()

  // ----------------------------------------------------
  // 1. CÁC PHÍM TẮT CẤP 2 (Chỉ chạy khi Dialog ĐANG MỞ)
  // ----------------------------------------------------
  const dialogShortcuts: KeyboardCommand[] = [
    {
      key: 'd',
      ctrl: true, // Ví dụ: Ctrl + D để chuyển nhanh sang Dark Mode khi đang mở Dialog
      action: () => console.log('Chuyển chế độ Dark mode!'),
    },
    {
      key: 'p',
      ctrl: true, // Ví dụ: Ctrl + P để đi tới trang Profile
      action: () => console.log('Đi tới trang cá nhân!'),
    },
    // Bạn định nghĩa thêm các combo phím phụ dành riêng cho Dialog ở đây...
  ]

  // Hàm xử lý phím tắt Cấp 2
  const handleDialogKeyDown = (event: KeyboardEvent) => {
    const pressedKey = event.key.toLowerCase()
    const isCtrlPressed = event.ctrlKey || event.metaKey
    const isShiftPressed = event.shiftKey
    const isAltPressed = event.altKey

    const matched = dialogShortcuts.find((s) => {
      return s.key.toLowerCase() === pressedKey && !!s.ctrl === isCtrlPressed && !!s.shift === isShiftPressed && !!s.alt === isAltPressed
    })

    if (matched) {
      event.preventDefault()
      matched.action()
      // commandPalette.close() // Thêm dòng này nếu muốn chạy lệnh xong thì tự đóng dialog
    }
  }

  // ----------------------------------------------------
  // 2. PHÍM TẮT CẤP 1 (Luôn lắng nghe toàn cục)
  // ----------------------------------------------------
  const handleGlobalKeyDown = (event: KeyboardEvent) => {
    const target = event.target as HTMLElement
    // Nếu đang gõ trong input (trừ ô search của chính dialog), không kích hoạt Ctrl+K ngoài ý muốn
    if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
      // Cho phép phím Escape hoạt động để đóng modal kể cả khi đang gõ
      if (event.key.toLowerCase() === 'escape' && commandPalette.isOpen) {
        commandPalette.close()
      }
      return
    }

    const pressedKey = event.key.toLowerCase()
    const isCtrlPressed = event.ctrlKey || event.metaKey

    // Bấm Ctrl + K để MỞ
    if (isCtrlPressed && pressedKey === 'k') {
      event.preventDefault()
      commandPalette.open()
    }

    // Bấm Space để play/pause
    if (event.key === ' ' || event.code === 'Space') {
      if (player.currentSong) {
        event.preventDefault()
        player.togglePlay()
      }
    }

    // Bấm ArrowLeft / ArrowRight để tua nhạc (seek)
    if (event.key === 'ArrowLeft') {
      if (player.currentSong) {
        event.preventDefault()
        if (event.altKey) {
          player.prevTrack()
        } else {
          const step = Number(player.seekStep)
          const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 5 : step
          const newTime = Math.max(0, (player.currentTime || 0) - finalStep)
          if (!isNaN(newTime) && isFinite(newTime)) {
            player.seek(newTime)
          }
        }
      }
    } else if (event.key === 'ArrowRight') {
      if (player.currentSong) {
        event.preventDefault()
        if (event.altKey) {
          player.nextTrack()
        } else {
          const step = Number(player.seekStep)
          const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 5 : step
          const newTime = Math.min(player.duration || 0, (player.currentTime || 0) + finalStep)
          if (!isNaN(newTime) && isFinite(newTime)) {
            player.seek(newTime)
          }
        }
      }
    }

    // Bấm ArrowUp / ArrowDown để tăng giảm âm lượng (volume)
    if (event.key === 'ArrowUp') {
      event.preventDefault()
      const step = Number(player.volumeStep)
      const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 2 : step
      player.volume = Math.min(100, (player.volume || 0) + finalStep)
    } else if (event.key === 'ArrowDown') {
      event.preventDefault()
      const step = Number(player.volumeStep)
      const finalStep = isNaN(step) || !isFinite(step) || step <= 0 ? 2 : step
      player.volume = Math.max(0, (player.volume || 0) - finalStep)
    }
  }

  // ----------------------------------------------------
  // 3. QUẢN LÝ VÒNG ĐỜI (Lifecycle & Watcher)
  // ----------------------------------------------------

  // Theo dõi trạng thái isOpen của Dialog để Đăng ký / Hủy đăng ký sự kiện cấp 2
  watch(
    () => commandPalette.isOpen,
    (isOpenNow) => {
      if (isOpenNow) {
        // Khi Dialog MỞ -> Bắt đầu lắng nghe các combo phím phụ
        window.addEventListener('keydown', handleDialogKeyDown)
        console.log('Đã kích hoạt bộ lắng nghe phím tắt Cấp 2 (Dialog mở)')
      } else {
        // Khi Dialog ĐÓNG -> Hủy lắng nghe ngay lập tức để tránh rò rỉ bộ nhớ
        window.removeEventListener('keydown', handleDialogKeyDown)
        console.log('Đã gỡ bỏ bộ lắng nghe phím tắt Cấp 2 (Dialog đóng)')
      }
    },
  )

  // Khởi tạo lắng nghe toàn cục (Chỉ dành cho Ctrl + K)
  const initShortcuts = () => {
    onMounted(() => {
      window.addEventListener('keydown', handleGlobalKeyDown)
    })

    onUnmounted(() => {
      window.removeEventListener('keydown', handleGlobalKeyDown)
      // Đảm bảo dọn dẹp sạch sẽ nếu component chứa bị unmount
      window.removeEventListener('keydown', handleDialogKeyDown)
    })
  }

  return {
    initShortcuts,
  }
}
