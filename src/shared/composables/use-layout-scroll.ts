import { OverlayScrollbars } from 'overlayscrollbars'
import { provide, inject, ref, watch, onUnmounted, type Ref, type InjectionKey } from 'vue'

export interface LayoutScrollContext {
  osInstance: Ref<any | null>
}

export const LAYOUT_SCROLL_KEY: InjectionKey<LayoutScrollContext> = Symbol('layoutScroll')

/**
 * Provides the OverlayScrollbars instance to child components.
 * To be called in the layout component (e.g. main-layout.vue).
 */
export function provideLayoutScroll(osInstance: Ref<any | null>) {
  provide(LAYOUT_SCROLL_KEY, {
    osInstance,
  })
}

/**
 * Subscribes to the layout's OverlayScrollbars scroll events.
 * Handles automatic cleanup on component unmount or if the instance changes.
 *
 * @param onScroll Callback function that runs when scrolling occurs.
 */
export function useLayoutScroll(onScroll?: (instance: OverlayScrollbars, event?: Event) => void) {
  const context = inject(LAYOUT_SCROLL_KEY, null)

  if (!context) {
    console.warn('useLayoutScroll must be used within a child component of main-layout')
    return {
      osInstance: ref(null),
    }
  }

  if (onScroll) {
    let unsubscribe: (() => void) | null = null

    const registerListener = (instance: any) => {
      if (unsubscribe) {
        unsubscribe()
        unsubscribe = null
      }
      if (instance) {
        // Run once initially to capture current scroll state
        onScroll(instance)

        // Register the scroll event
        unsubscribe = instance.on('scroll', (inst: any, event: Event) => {
          onScroll(inst, event)
        })
      }
    }

    watch(context.osInstance, registerListener, { immediate: true })

    onUnmounted(() => {
      if (unsubscribe) {
        unsubscribe()
      }
    })
  }

  return {
    osInstance: context.osInstance,
  }
}
