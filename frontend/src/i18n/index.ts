import { computed, ref } from 'vue'
import { detectLocale, translate, type Locale } from './detect'
import { messages } from './messages'

export type { Locale }

const STORAGE_KEY = 'pmusim.locale'

const locale = ref<Locale>(
  detectLocale(
    typeof navigator !== 'undefined' ? navigator.language : undefined,
    typeof localStorage !== 'undefined' ? localStorage.getItem(STORAGE_KEY) : null,
  ),
)

export function setLocale(l: Locale): void {
  locale.value = l
  try { localStorage.setItem(STORAGE_KEY, l) } catch { /* private mode etc. */ }
}

/** Standalone translator — reads the reactive locale, so it is reactive in
 *  templates/computed AND usable from non-component modules (composables). */
export function t(key: string, params?: Record<string, string | number>): string {
  return translate(messages[locale.value], key, params)
}

export function useI18n() {
  return { locale: computed(() => locale.value), setLocale, t }
}
