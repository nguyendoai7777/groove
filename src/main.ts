import { createApp } from 'vue'
import App from './App.vue'
import { createVuetify } from 'vuetify'
import { createPinia } from 'pinia'
import 'vuetify/styles'
import 'overlayscrollbars/overlayscrollbars.css'
import './tailwind.css'
import './style.css'
import { AppRouter } from './app.route'
import { VBtn, VCard, VDialog, VList, VListItem, VTextField } from 'vuetify/components'

const app = createApp(App)
const vtx = createVuetify({
  components: {
    'v-btn': VBtn,
    'v-dialog': VDialog,
    'v-card': VCard,
    'v-text-field': VTextField,
    'v-list': VList,
    'v-list-item': VListItem,
  },
})
app.use(AppRouter).use(createPinia()).use(vtx).mount('#app')
