import { createApp } from "vue";
import App from "./App.vue";
import { usePmuEvents } from "./composables/usePmuEvents";

// Tauri 2's `listen()` IPC needs the webview to be fully ready, which
// only happens AFTER createApp().mount() — calling it earlier (top-level
// `await listen()` or even synchronously here) deadlocks because the
// webview message channel isn't yet plumbed. So we mount first, then
// fire startListening(). Components that issue commands await
// `listenerReady` from usePmuEvents to ensure the listener is attached
// before the first invoke (otherwise handshake events drop on the floor).
createApp(App).mount("#app");
usePmuEvents().startListening();
