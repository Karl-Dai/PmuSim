import { ref } from "vue";

// Shared "is master server running" flag. ToolbarPanel owns transitions
// (start_server / stop_server); StationListPanel reads it to gate the
// connect button so users can't fire connect_substation while the Rust
// side master is None (which would otherwise produce a stream of
// "Server not running" toasts after every click).
const running = ref(false);

export function useServerStatus() {
  return { running };
}
