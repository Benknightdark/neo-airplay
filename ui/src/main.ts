import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import { router } from './router'
import './styles.css'

if (import.meta.env.VITE_TAURI_E2E === 'true') {
  await import('@wdio/tauri-plugin')
}

createApp(App).use(createPinia()).use(router).mount('#app')
