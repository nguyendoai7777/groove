import { ref } from 'vue'

export interface ToastMessage {
  id: string
  message: string
  duration?: number
}

const toasts = ref<ToastMessage[]>([])

export function useToast() {
  function show(message: string, duration = 4000) {
    const id = Math.random().toString(36).substring(2, 9)
    const toast: ToastMessage = { id, message, duration }
    toasts.value.push(toast)
    setTimeout(() => {
      remove(id)
    }, duration)
  }

  function remove(id: string) {
    toasts.value = toasts.value.filter((t) => t.id !== id)
  }

  return {
    toasts,
    show,
    remove,
  }
}
