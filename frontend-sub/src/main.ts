import { createApp } from "vue";
import App from "./App.vue";
import { useSubEvents } from "./composables/useSubEvents";

createApp(App).mount("#app");
useSubEvents().startListening();
