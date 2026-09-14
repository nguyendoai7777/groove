import { createApp } from 'vue';
import App from './App.vue';
import { createVuetify } from 'vuetify';
import { createPinia } from 'pinia';
import 'vuetify/styles';
import 'overlayscrollbars/overlayscrollbars.css';
import './tailwind.css';
import './style.css';
import { AppRouter } from './app.route';
import {
  VBtn,
  VCard,
  VCardActions,
  VCardText,
  VCardTitle,
  VDialog,
  VList,
  VListItem,
  VListItemTitle,
  VMenu,
  VProgressCircular,
  VSlider,
  VTab,
  VTabs,
  VTabsWindow,
  VTabsWindowItem,
  VTextField,
} from 'vuetify/components';

const app = createApp(App);
const vtx = createVuetify({
  components: {
    'v-btn': VBtn,
    'v-dialog': VDialog,
    'v-card': VCard,
    'v-card-actions': VCardActions,
    'v-card-title': VCardTitle,
    'v-card-text': VCardText,
    'v-text-field': VTextField,
    'v-list': VList,
    'v-list-item': VListItem,
    'v-tabs': VTabs,
    'v-tab': VTab,
    'v-tabs-window': VTabsWindow,
    'v-tabs-window-item': VTabsWindowItem,
    'v-progress-circular': VProgressCircular,
    'v-slider': VSlider,
    'v-menu': VMenu,
    'v-list-item-title': VListItemTitle,
  },
});
app.use(AppRouter).use(createPinia()).use(vtx).mount('#app');

// Boot timing, so a slow start can be attributed instead of guessed at. `startTime`
// is when the document began loading, so this covers script download, parse and
// execution as well as Vue's first render. Open devtools and read the two numbers:
// a large "bundle ready" means the front end is the cost, while a small one with a
// slow-feeling window points at window compositing (transparency + blur) instead.
if (import.meta.env.DEV || location.search.includes('boot-timing')) {
  const nav = performance.getEntriesByType('navigation')[0] as PerformanceNavigationTiming | undefined;
  requestAnimationFrame(() => {
    const now = Math.round(performance.now());
    console.info(`[boot] bundle ready ${Math.round(nav?.domContentLoadedEventEnd ?? 0)}ms · first frame ${now}ms`);
  });
}
