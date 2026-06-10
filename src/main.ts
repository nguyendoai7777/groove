import { createApp } from 'vue'
import App from './App.vue'
import { createVuetify } from 'vuetify'
import { createPinia } from 'pinia'
import 'vuetify/styles'
import './tailwind.css'
import './style.css'
import { AppRouter } from './app.route'
import { VBtn } from 'vuetify/components'

const app = createApp(App)
const vtx = createVuetify({
  components: {
    'v-btn': VBtn,
  },
})
app.use(AppRouter).use(createPinia()).use(vtx).mount('#app')
