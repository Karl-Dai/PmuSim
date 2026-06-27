# 主站时标延迟极值读数 — 设计

## 背景与目标

主站已有「本地时间偏差」读数:后端逐帧算 `local_offset_ms = now_unix_ms −
frame_abs_ms`(正=报文滞后本地,负=超前),前端 `useTimeOffset` 在 50 帧定长
滑动窗口里求均值,读数面板显示单个 `+xx ms`。

本需求在同一窗口、同一样本上再加两个聚合:**最大值**与**最小值**,反映该窗口
内报文时标相对本机时钟的峰值偏移(最大滞后 / 最小滞后或超前)。均值仍保留。

- **语义**:复用现有 `local_offset_ms` 样本,取 `max` / `min`。不做绝对值、不
  只取正向。
- **窗口**:与均值同源 50 帧定长滑动窗口,断开/StreamingStopped/HeartbeatTimeout
  一并重置(复用现有 `reset`)。
- **展示**:读数面板「本地时间偏差」行下新增两行「最大时标延迟」「最小时标
  延迟」,格式与现有一致。
- **范围**:纯前端改动,后端零改动。仅实时读数,不告警、不进数据表、不画曲线。

## 改动

### `useTimeOffset.ts`

窗口 `windows: Map<idcode, number[]>` 与 `offsetMap`(均值)不变。新增两个
reactive map:

```ts
const maxMap = reactive(new Map<string, number | null>());
const minMap = reactive(new Map<string, number | null>());
```

`tick(idcode, ms)`:

1. 样本入队(逻辑同现状)。
2. 窗口长度 0→1 时:`maxMap`/`minMap` 置为新样本。
3. 否则:`maxMap = Math.max(curMax, ms)`、`minMap = Math.min(curMin, ms)`。
4. 出队(`length > WINDOW` shift)后,**若离开的样本恰为当前极值**,重扫窗口
   重算(50 帧,一次 O(N),均摊 O(1)):`maxMap = Math.max(...samples)`、
   `minMap = Math.min(...samples)`;否则极值不变。
5. 均值逻辑不变。

`reset(idcode)`:`windows`、`offsetMap`、`maxMap`、`minMap` 四者一并清空
(`maxMap.set(idcode, null)` 等,口径与现状 `offsetMap` 一致)。

新增访问器,镜像 `offsetOf`:

```ts
function maxOf(idcode: string): number | null { return maxMap.get(idcode) ?? null; }
function minOf(idcode: string): number | null { return minMap.get(idcode) ?? null; }
```

返回 `{ tick, reset, offsetOf, maxOf, minOf }`。

> `Math.max(...samples)` 在 N=50 下无栈溢出风险;若将来窗口调大,改用 reduce。

### `ConfigInfoPanel.vue`

读数面板「本地时间偏差」行下加两行:

```
最大时标延迟: +xx ms
最小时标延迟: +xx ms
```

- 抽 `fmtSignedMs(v: number | null): string` 纯函数:无样本 `—`,否则带符号整数
  (`+12` / `−1450`,正号显式,负号由数字自带)。`clockOffsetText` / `maxText` /
  `minText` 三处共用,避免重复。
- `maxMs` / `minMs` computed,取 `maxOf(selectedIdcode.value)` / `minOf(...)`。
- `maxText` / `minText` computed,套 `fmtSignedMs`。
- 两行结构与 `本地时间偏差` 行一致(`rd-row` / `rd-val mono` / `unit`),无样本
  显示 `—`。

### i18n

`messages.ts` 新增(中英):

- `config.maxLatency`: zh「最大时标延迟」 / en「Max latency」
- `config.minLatency`: zh「最小时标延迟」 / en「Min latency」

`config.msUnit`(zh/en 均「ms」)已存在,复用。

## 数据流

不变。`DataFrame` 事件 → `usePmuEvents.handle` 的 `DataFrame` 分支调
`tickOffset(idcode, local_offset_ms)` → `useTimeOffset` 内部更新均值 + max + min
三个 map → `ConfigInfoPanel` computed 取值 → 模板渲染。`usePmuEvents` 无改动
(reset 口径已覆盖三个 map)。

## 测试

扩写 `frontend/tests/use-time-offset.test.ts`:

- `maxOf` / `minOf` 随样本更新,正负保留(如 +10 / −10 / +30 → max=30, min=−10,
  均值=10 仍对)。
- 滑出极值样本后重扫正确:构造 >50 帧使峰值滑出窗口,max 回落到窗口内新峰值;
  min 同理。
- 两子站隔离:`maxOf("A")` 与 `maxOf("B")` 互不串台。
- `reset` 后 `maxOf` / `minOf` 归 `null`。
- 未知 idcode `maxOf("nope")` 返回 `null`。

无后端测试改动。

## 不做(YAGNI)

- 不超阈告警(无 toast / 事件日志 / 语义色)。
- 不进数据表、不画历史曲线、不存档。
- 不另开更长窗口。
- 不动后端 / 不新增事件类型。
- 不改子站生成侧。
