import { ref } from "vue";

// Single shared protocol selection used by ConfigInfoPanel and any future
// consumers. Module-level so the value survives v-if re-mounts.
export type Protocol = "V2" | "V3";

const protocol = ref<Protocol>("V3");

export function useProtocol() {
  return { protocol };
}
