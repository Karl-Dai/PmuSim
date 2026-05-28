import { ref } from "vue";

// Shared "is master server running" flag. ConfigInfoPanel sets it on
// start_server / stop_server and reads it to gate all connection / data
// commands (otherwise the Rust side rejects every call with "Server not
// running" and the UI floods toasts).
const running = ref(false);

export function useServerStatus() {
  return { running };
}
