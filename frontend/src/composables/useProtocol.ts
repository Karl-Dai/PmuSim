import { ref } from "vue";

// Single shared protocol selection across the toolbar + station list panel.
// Lifting it out of either panel ensures the data-port field in
// StationListPanel responds to the toolbar's protocol toggle.
export type Protocol = "V2" | "V3";

const protocol = ref<Protocol>("V3");

export function useProtocol() {
  return { protocol };
}
