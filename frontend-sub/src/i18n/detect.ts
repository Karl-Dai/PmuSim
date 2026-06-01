// Pure, Vue-free i18n helpers. Unit-tested standalone via
// `node --experimental-strip-types` (no test-runner dependency).

export type Locale = 'zh' | 'en'

/** First-run resolution: an explicit stored choice wins; otherwise infer
 *  from the OS/webview locale (zh* → zh, everything else → en). */
export function detectLocale(navLang: string | undefined, stored: string | null): Locale {
  if (stored === 'zh' || stored === 'en') return stored
  return (navLang ?? '').toLowerCase().startsWith('zh') ? 'zh' : 'en'
}

/** Look up `key` in a single-locale catalog and interpolate `{var}`
 *  placeholders. Missing key → returns the key (and warns in dev). */
export function translate(
  catalog: Record<string, string>,
  key: string,
  params?: Record<string, string | number>,
): string {
  const raw = catalog[key]
  if (raw === undefined) {
    if (import.meta.env?.DEV) console.warn(`[i18n] missing key: ${key}`)
    return key
  }
  if (!params) return raw
  return raw.replace(/\{(\w+)\}/g, (_, name) =>
    name in params ? String(params[name]) : `{${name}}`,
  )
}
