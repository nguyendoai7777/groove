import { createApp } from 'vue'
import App from './App.vue'
import { createVuetify } from 'vuetify'
import { createPinia } from 'pinia'
import 'vuetify/styles'
import 'overlayscrollbars/overlayscrollbars.css'
import './tailwind.css'
import './style.css'
import { AppRouter } from './app.route'
import {
  VBtn,
  VCard,
  VDialog,
  VList,
  VListItem,
  VProgressCircular,
  VSlider,
  VTab,
  VTabs,
  VTabsWindow,
  VTabsWindowItem,
  VTextField,
} from 'vuetify/components'

const app = createApp(App)
const vtx = createVuetify({
  components: {
    'v-btn': VBtn,
    'v-dialog': VDialog,
    'v-card': VCard,
    'v-text-field': VTextField,
    'v-list': VList,
    'v-list-item': VListItem,
    'v-tabs': VTabs,
    'v-tab': VTab,
    'v-tabs-window': VTabsWindow,
    'v-tabs-window-item': VTabsWindowItem,
    'v-progress-circular': VProgressCircular,
    'v-slider': VSlider,
  },
})
app.use(AppRouter).use(createPinia()).use(vtx).mount('#app')
