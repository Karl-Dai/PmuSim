import assert from 'node:assert/strict'
import { messages } from '../../src/i18n/messages.ts'

const zh = Object.keys(messages.zh).sort()
const en = Object.keys(messages.en).sort()
const onlyZh = zh.filter((k) => !(k in messages.en))
const onlyEn = en.filter((k) => !(k in messages.zh))
assert.deepEqual(onlyZh, [], `keys missing from en: ${onlyZh}`)
assert.deepEqual(onlyEn, [], `keys missing from zh: ${onlyEn}`)
assert.ok(zh.length > 40, `expected the full catalog, got ${zh.length} keys`)

console.log(`messages-parity.test.mjs OK (${zh.length} keys)`)
