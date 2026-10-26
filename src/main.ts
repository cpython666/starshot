import { createApp } from 'vue'
import './styles.css'

const view = new URLSearchParams(location.search).get('view')
const component = view === 'capture' && '__TAURI_INTERNALS__' in window
  ? import('./components/CaptureOverlay.vue')
  : view === 'pin' ? import('./components/PinWindow.vue') : import('./App.vue')
component.then(({ default: app }) => createApp(app).mount('#app'))
