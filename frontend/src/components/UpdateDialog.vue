<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useI18n } from '../i18n'

const props = defineProps<{
  visible: boolean
  version: string
  notes: string
}>()
const emit = defineEmits<{
  (e: 'close'): void
}>()

const { t } = useI18n()

const busy = ref(false)
const error = ref<string | null>(null)

type Span = { text: string; bold?: boolean; code?: boolean }
type Block =
  | { kind: 'h'; level: number; spans: Span[] }
  | { kind: 'li'; spans: Span[] }
  | { kind: 'p'; spans: Span[] }
  | { kind: 'quote'; spans: Span[] }
  | { kind: 'hr' }

function parseInline(text: string): Span[] {
  const spans: Span[] = []
  const re = /(\*\*[^*]+\*\*|`[^`]+`)/g
  let last = 0
  let m: RegExpExecArray | null
  while ((m = re.exec(text)) !== null) {
    if (m.index > last) spans.push({ text: text.slice(last, m.index) })
    const tok = m[0]
    if (tok.startsWith('**')) spans.push({ text: tok.slice(2, -2), bold: true })
    else spans.push({ text: tok.slice(1, -1), code: true })
    last = m.index + tok.length
  }
  if (last < text.length) spans.push({ text: text.slice(last) })
  return spans.length ? spans : [{ text }]
}

const noteBlocks = computed<Block[]>(() => {
  const out: Block[] = []
  for (const raw of (props.notes || '').split('\n')) {
    const line = raw.trim()
    if (!line) continue
    if (/^[-*_]{3,}$/.test(line)) { out.push({ kind: 'hr' }); continue }
    const h = line.match(/^(#{1,6})\s+(.*)$/)
    if (h) { out.push({ kind: 'h', level: h[1].length, spans: parseInline(h[2]) }); continue }
    const li = line.match(/^[-*]\s+(.*)$/)
    if (li) { out.push({ kind: 'li', spans: parseInline(li[1]) }); continue }
    const q = line.match(/^>\s?(.*)$/)
    if (q) { out.push({ kind: 'quote', spans: parseInline(q[1]) }); continue }
    out.push({ kind: 'p', spans: parseInline(line) })
  }
  return out
})

async function runAction(command: string, closeAfter: boolean) {
  if (busy.value) return
  error.value = null
  busy.value = true
  try {
    const args = command === 'install_update' ? undefined : { version: props.version }
    await invoke(command, args)
    if (closeAfter) emit('close')
  } catch (e: any) {
    error.value = String(e)
  } finally {
    busy.value = false
  }
}

function installNow() {
  return runAction('install_update', false)
}

function skip() {
  return runAction('skip_update', true)
}

function installOnNextLaunch() {
  return runAction('schedule_update_on_next_launch', true)
}

function onBackdrop() {
  if (busy.value) return
  if (error.value) emit('close')
  else void skip()
}

function onKeydown(e: KeyboardEvent) {
  if (props.visible && e.key === 'Escape') onBackdrop()
}
onMounted(() => window.addEventListener('keydown', onKeydown))
onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKeydown)
})
</script>

<template>
  <Teleport to="body">
    <Transition name="dialog-pop">
      <div v-if="visible" class="upd-backdrop" @mousedown.self="onBackdrop">
        <div class="upd-dialog" role="dialog" aria-modal="true" aria-labelledby="upd-title">
          <div class="upd-header">
            <div class="upd-titles">
              <div id="upd-title" class="upd-title">{{ t("update.title") }}</div>
              <div class="upd-subtitle">{{ t("update.subtitle", { version }) }}</div>
            </div>
            <span class="upd-badge">v{{ version }}</span>
          </div>

          <div class="upd-body">
            <div class="upd-section-label">{{ t("update.changelog") }}</div>
            <div class="upd-ready" role="status">{{ t("update.ready") }}</div>
            <div class="upd-notes" tabindex="0">
              <template v-for="(blk, i) in noteBlocks" :key="i">
                <hr v-if="blk.kind === 'hr'" class="upd-hr" />
                <component
                  :is="'h' + Math.min(blk.level + 2, 6)"
                  v-else-if="blk.kind === 'h'"
                  class="upd-h"
                >
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </component>
                <div v-else-if="blk.kind === 'li'" class="upd-li">
                  <span class="upd-bullet" aria-hidden="true"></span>
                  <span class="upd-li-text">
                    <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                      <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                      <template v-else>{{ s.text }}</template>
                    </span>
                  </span>
                </div>
                <blockquote v-else-if="blk.kind === 'quote'" class="upd-quote">
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </blockquote>
                <p v-else class="upd-p">
                  <span v-for="(s, j) in blk.spans" :key="j" :class="{ b: s.bold }">
                    <code v-if="s.code" class="upd-code">{{ s.text }}</code>
                    <template v-else>{{ s.text }}</template>
                  </span>
                </p>
              </template>
            </div>

            <div v-if="error" class="upd-error" role="alert">
              <div class="upd-error-title">{{ t("update.failedTitle") }}</div>
              <pre class="upd-error-msg">{{ error }}</pre>
            </div>
          </div>

          <div class="upd-footer">
            <span v-if="busy" class="upd-footer-hint">{{ t("update.working") }}</span>
            <button class="btn btn-ghost" :disabled="busy" @click="skip">{{ t("update.skip") }}</button>
            <button class="btn btn-secondary" :disabled="busy" @click="installOnNextLaunch">
              {{ t("update.installNextLaunch") }}
            </button>
            <button class="btn btn-primary" :disabled="busy" @click="installNow">
              {{ t("update.installNow") }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.upd-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(17, 17, 27, 0.55);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 2100;
}
.upd-dialog {
  display: flex;
  flex-direction: column;
  background: #f5f5f0;
  border: 1px solid #b0b0a8;
  border-radius: 6px;
  width: 540px;
  max-width: 92vw;
  max-height: 78vh;
  box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
  overflow: hidden;
}
.upd-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 12px;
  padding: 12px 16px 10px;
  border-bottom: 1px solid #d0d0c8;
  background: linear-gradient(#eaeae0, #dcdcd0);
}
.upd-title { font-size: 14px; font-weight: 700; color: #1a1a1a; }
.upd-subtitle { font-size: 12px; color: #555; margin-top: 3px; }
.upd-badge {
  flex-shrink: 0;
  font-size: 12px;
  font-weight: 600;
  font-variant-numeric: tabular-nums;
  color: #1a4f8b;
  background: rgba(90, 140, 204, 0.18);
  border: 1px solid rgba(90, 140, 204, 0.4);
  padding: 2px 8px;
  border-radius: 999px;
}
.upd-body { padding: 12px 16px; overflow-y: auto; }
.upd-section-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: #666;
  margin-bottom: 6px;
}
.upd-ready {
  margin-bottom: 8px;
  padding: 7px 9px;
  border: 1px solid #a7cfac;
  border-radius: 4px;
  background: #edf8ef;
  color: #246b31;
  font-size: 12px;
}
.upd-notes {
  background: #fff;
  border: 1px solid #cfcfc6;
  border-radius: 4px;
  padding: 10px 12px;
  max-height: 320px;
  overflow-y: auto;
  font-size: 12px;
  line-height: 1.6;
  color: #333;
}
.upd-notes:focus-visible { outline: 2px solid #5a8ccc; outline-offset: -1px; }
.upd-h {
  margin: 12px 0 4px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: #2c5a99;
}
.upd-h:first-child { margin-top: 0; }
.upd-p { margin: 4px 0; }
.upd-li { display: flex; gap: 8px; margin: 4px 0; align-items: baseline; }
.upd-bullet {
  flex-shrink: 0;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: #2c5a99;
  transform: translateY(-1px);
}
.upd-li-text { flex: 1; min-width: 0; }
.upd-notes .b { font-weight: 600; color: #111; }
.upd-code {
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11.5px;
  color: #a05500;
  background: #f4ede0;
  border: 1px solid #d9cba8;
  border-radius: 3px;
  padding: 0 4px;
}
.upd-quote {
  margin: 6px 0;
  padding: 4px 10px;
  border-left: 2px solid #b8b8b0;
  color: #555;
}
.upd-hr { border: none; border-top: 1px solid #d0d0c8; margin: 10px 0; }
.upd-error {
  margin-top: 12px;
  padding: 8px 10px;
  background: #fde8e8;
  border: 1px solid #f5b5b5;
  border-radius: 4px;
}
.upd-error-title { font-size: 13px; font-weight: 600; color: #9b1c1c; }
.upd-error-msg {
  margin: 4px 0 0;
  white-space: pre-wrap;
  word-break: break-word;
  font-family: ui-monospace, "SF Mono", Menlo, monospace;
  font-size: 11px;
  color: #5a1717;
}
.upd-footer {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 8px;
  padding: 10px 16px;
  border-top: 1px solid #d0d0c8;
  background: linear-gradient(#e0e0d4, #d4d4c8);
}
.upd-footer-hint { font-size: 12px; color: #555; }
.btn {
  padding: 6px 16px;
  border: 1px solid transparent;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  transition: background 140ms ease, border-color 140ms ease;
}
.btn:disabled { cursor: wait; opacity: 0.55; }
.btn:focus-visible { outline: 2px solid #5a8ccc; outline-offset: 2px; }
.btn-primary { background: linear-gradient(#5a8ccc, #2c5a99); color: #fff; border-color: #1d4377; }
.btn-primary:hover { background: linear-gradient(#6a9cdc, #3c6aa9); }
.btn-secondary { background: #d9e5f4; color: #1d4377; border-color: #8aa8cc; }
.btn-secondary:hover { background: #e8f0f9; }
.btn-ghost { background: #ebebe2; color: #333; border-color: #b8b8b0; }
.btn-ghost:hover { background: #f5f5ec; }
</style>
