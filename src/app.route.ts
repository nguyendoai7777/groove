import { createRouter, createWebHistory, RouteRecordRaw } from 'vue-router'
import MyMusic from '@groovex/pages/my-music/my-music.vue'
import NowPlaying from '@groovex/pages/now-playing/now-playing.vue'
import Playlists from '@groovex/pages/playlists/playlists.vue'
import MainLayout from './layouts/main-layout.vue'

export const APP_ROUTES: RouteRecordRaw[] = [
  {
    path: '/',
    component: MainLayout,
    children: [
      { path: '', redirect: 'my-music' },
      { path: 'my-music', component: MyMusic, meta: { title: 'My Music' } },
      { path: 'playing', component: NowPlaying, meta: { title: 'Now Playing' } },
      { path: 'playlists', component: Playlists, meta: { title: 'Playlists' } },
    ],
  },
]

export const AppRouter = createRouter({
  history: createWebHistory(),
  routes: APP_ROUTES,
})

AppRouter.beforeEach((to) => {
  if (to.meta && typeof to.meta.title === 'string') {
    document.title = `${to.meta.title} | GrooveX`
  } else {
    document.title = 'GrooveX'
  }
})
