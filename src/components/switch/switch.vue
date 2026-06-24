<template>
  <label
    class="inline-flex items-center cursor-pointer select-none touch-none"
    @mousedown="isPressing = true"
    @mouseup="isPressing = false"
    @mouseleave="isPressing = false"
    @touchstart="isPressing = true"
    @touchend="isPressing = false">
    <input type="checkbox" v-model="modelValue" class="sr-only" />
    <div class="relative transition-colors duration-300 ease-in-out" :class="modelValue ? activeColor : inactiveColor" :style="trackStyle">
      <div
        class="absolute bg-white rounded-full shadow-md transition-all duration-300 ease-out will-change-[width,transform]"
        :style="thumbStyle"></div>
    </div>
  </label>
</template>

<script setup lang="ts">
  import { ref, computed } from 'vue';
  interface Props {
    /** Kích thước của ô tròn switch */
    size?: number;
    /** Chiều rộng của track switch */
    width?: number;
    /** Màu sắc khi switch được bật */
    activeColor?: string;
    /** Màu sắc khi switch được tắt */
    inactiveColor?: string;
  }

  const props = withDefaults(defineProps<Props>(), {
    size: 24,
    width: undefined,
    activeColor: 'bg-green-500',
    inactiveColor: 'bg-gray-300',
  });
  const modelValue = defineModel<boolean>({ default: false });
  const isPressing = ref(false);

  const PADDING = 2;

  const actualTrackWidth = computed(() => {
    if (props.width) return props.width;
    return Math.round(props.size * 1.88);
  });

  const trackStyle = computed(() => {
    const height = props.size + PADDING * 2;

    return {
      width: `${actualTrackWidth.value}px`,
      height: `${height}px`,
      borderRadius: `${height / 2}px`,
    };
  });

  const thumbStyle = computed(() => {
    const d = props.size;
    const currentWidth = isPressing.value ? Math.round(d * 1.25) : d;
    let translateX = 0;
    if (modelValue.value) {
      translateX = actualTrackWidth.value - PADDING - currentWidth - PADDING;
    }

    return {
      top: `${PADDING}px`,
      left: `${PADDING}px`,
      height: `${d}px`,
      width: `${currentWidth}px`,
      transform: `translateX(${translateX}px)`,
    };
  });
</script>

<script lang="ts">
  /**
   * 📘 COMPONENT SWITCH (BẬT/TẮT)
   * Hướng dẫn: Dùng để thay đổi trạng thái On/Off.
   * @example
   * ```vue
   * <Switch v-model="status" :size="30" activeColor="bg-blue-500" />
   * ```
   */
  export default {};
</script>
