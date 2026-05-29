import assert from 'node:assert/strict'
import { detectLocale, translate } from '../../src/i18n/detect.ts'

// detectLocale: explicit stored choice wins over OS locale
assert.equal(detectLocale('en-US', 'zh'), 'zh')
assert.equal(detectLocale('zh-CN', 'en'), 'en')
// detectLocale: fall back to OS locale when nothing stored
assert.equal(detectLocale('zh-CN', null), 'zh')
assert.equal(detectLocale('zh', null), 'zh')
assert.equal(detectLocale('en-US', null), 'en')
assert.equal(detectLocale('fr-FR', null), 'en')
assert.equal(detectLocale(undefined, null), 'en')
// detectLocale: ignore a garbage stored value, use OS locale
assert.equal(detectLocale('zh-CN', 'klingon'), 'zh')

// translate: plain hit
assert.equal(translate({ 'a.b': 'hello' }, 'a.b'), 'hello')
// translate: {var} interpolation
assert.equal(translate({ k: 'hi {name}, {n} msgs' }, 'k', { name: 'X', n: 3 }), 'hi X, 3 msgs')
// translate: missing param left as literal token
assert.equal(translate({ k: 'a {x} b' }, 'k', {}), 'a {x} b')
// translate: missing key returns the key itself
assert.equal(translate({}, 'no.such.key'), 'no.such.key')

console.log('detect.test.mjs OK')
