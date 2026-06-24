import { defineStore } from 'pinia';
import { EStoreKey } from '@groovex/store';

export const useCommandPaletteStore = defineStore(EStoreKey.CommandPalette, {
  state: () => ({
    isOpen: false,
    searchQuery: '',
  }),
  actions: {
    toggle() {
      this.isOpen = !this.isOpen;
    },
    open() {
      this.isOpen = true;
    },
    close() {
      this.isOpen = false;
      this.searchQuery = ''; // Reset khi đóng
    },
  },
});
