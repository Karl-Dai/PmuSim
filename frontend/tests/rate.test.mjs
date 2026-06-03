import assert from 'node:assert/strict'
import { hzToPeriod } from '../src/lib/rate.ts'

// 正常档位：PERIOD = round(5000/hz)
assert.equal(hzToPeriod(25), 200)
assert.equal(hzToPeriod(50), 100)
assert.equal(hzToPeriod(100), 50)
assert.equal(hzToPeriod(200), 25)
// 0Hz 特判 → PERIOD=0（非法上送周期，绕开 1000/hz 除零）
assert.equal(hzToPeriod(0), 0)

console.log('rate.test.mjs OK')
