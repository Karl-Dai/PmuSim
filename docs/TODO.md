# TODO — PMU V3 规约符合性查漏补缺

来源:`/code-review 根据规约查漏补缺`(2026-05-28)。
对照规约:`PMU 协议 V3 版报文解析指导手册`(§8.1–8.13)。

修复优先级 = 现场触发概率 × 严重度 / 工作量。`P0` 必须修,`P1` 接新子站前修,`P2` 现场调试期再说。

---

## P0 — 接入新子站立即触发

### [x] 1. FORMAT 标志位被忽略 → 浮点 / 直角坐标子站静默错码
- **位置:** `crates/pmusim-core/src/protocol/parser.rs:234` (`parse_data`)
- **现状:** PHASORS / ANALOG / FREQ / DFREQ 永远按 `i16` 读取,不看 CFG 中 FORMAT bit 0-3。
- **规约:** §8.5 表 8 FORMAT
  - Bit0=0 直角坐标 / =1 极坐标
  - Bit1=0 i16 / =1 float (相量)
  - Bit2=0 i16 / =1 float (模拟量)
  - Bit3=0 i16 / =1 float (FREQ/DFREQ)
- **触发场景:** 子站 FORMAT=0x001E(全 float)时,模拟量 4 字节按 2×i16 读,数量翻倍 + 数值完全错乱。当前 lab IEMP 子站 FORMAT=0x10 全 int16 + 极坐标(bit0=1)恰好命中默认路径所以未暴露。
- **修复:** `parse_data` 接收 `format_flags`,按位分支。需要相应改 `build_data` 与所有调用点。

### [x] 2. Multi-PMU 配置帧仅解第一台
- **位置:** `crates/pmusim-core/src/protocol/parser.rs:121` (`parse_config` 注释 "first PMU only for now")
- **规约:** §8.2 表 5 NUM_PMU 允许 > 1
- **触发:** NUM_PMU > 1 时,第一台 PMU 解完后偏移指向第二台 STN,被当成 PHUNIT 读 → 字段全乱。
- **修复:** 循环 NUM_PMU 次,改 `ConfigFrame` 为 `Vec<PmuBlock>`。改造面大,但接 ≥2 台子站必须。

### [x] 3. CFG-2 ACK / NACK 不等待 + 不响应
- **位置:**
  - `crates/pmusim-app/src/network/master.rs:966` — "ACK / NACK are informational - no state change needed"
  - `crates/pmusim-app/src/network/master.rs:1224` — `do_auto_handshake` 用固定 `sleep(500ms)` 替代等 ACK
- **规约:** §8.4 / §8.6 — 子站收到下传 CFG-2 命令、CFG-2 帧后必须回 ACK(0xE000)或 NACK(0x2000),master 必须收到 ACK 才能进入下一步。
- **触发:** 子站对 CFG-2 返回 NACK(配置不兼容),master 不解释,继续推 SendCfg2 + OpenData,UI 显示 Streaming 但数据帧永远不到。
- **修复:** state machine 加 `WaitingAck`,NACK → 切回 Cfg1Received + emit Error。

---

## P1 — 链路抖动 / 现场异常时翻车

### [x] 4. 心跳计数器 off-by-one — 实际 2 次未响应就断
- **位置:** `crates/pmusim-app/src/network/master.rs:460`
- **现状:** `session.missed_heartbeats += 1` 在 send 后无条件递增,与 `mgmt_read_loop` 收到回包置零发生竞态。稳态下 `missed_heartbeats=1`(因为 ++ 总是发生在收到响应之后),所以阈值 `>= 3` 实际只需 2 次连续未响应。
- **触发:** 链路 flap 一两次就断开。
- **修复:** 把"已发未确认"改成基于 `last_heartbeat` 时间差判断,或在 send 时记录 `pending_heartbeat_soc`,收到 echo 匹配后才清零。

### [x] 5. STAT bit10 (子站配置改变) 不触发重新召唤 CFG
- **位置:** `crates/pmusim-app/src/network/master.rs:1000`
- **规约:** §8.11 表 12 — bit10=1 表示子站配置在 1 min 内变更
- **触发:** 子站重新加载 CFG-2(通道数变化),master 用旧 dims 继续解,后续帧 size 对不上 → CRC fail → 静默丢帧,UI 还显示 Streaming。
- **修复:** `data_read_loop_outbound` 检测 STAT bit10,emit `ConfigChanged` 事件并自动 `auto_handshake`。

### [x] 6. `do_send_cmd` IO 错误吞了不切状态
- **位置:** `crates/pmusim-app/src/network/master.rs:1051-1053`
- **现状:** `write_all` 失败只 log + return,session.state 不变,也不 emit Error。
- **触发:** 子站 RST 之后,heartbeat_loop 每 30s 重试一次失败,UI 维持"在线"假象直到 `mgmt_read_loop` 因 EOF 才真清理(可能延迟数秒)。
- **修复:** write 失败立刻 `state = Disconnected` + `emit Error` + 关 writer。

### [x] 7. CFG-2 channel_names 长度不校验
- **位置:** `crates/pmusim-app/src/network/master.rs:1108` (`do_send_cfg2` 从 cfg1 复制 channel_names)
- **规约:** §6 "主站宜具有 CFG1/CFG2 配置帧的校验机制"
- **触发:** cfg1.channel_names.len() ≠ phnmr + annmr + 16 × dgnmr,builder 仍序列化 → CFG-2 字节布局错位 → 子站 CRC fail 丢弃 → 死锁。
- **修复:** `do_send_cfg2` 校验 + 不通过则 emit Error。

### [x] 8. ANUNIT 高字节 (类型码) 当数值用
- **位置:** `crates/pmusim-core/src/protocol/frame.rs:43` (`analog_factor`)
- **规约:** 规约 §8.2 表 5 字面写"原码 × 0.00001",未拆字节。但 V3 继承自 IEEE C37.118,高字节是模拟量类型(0=单点波形 / 1=RMS / 2=峰值)。
- **触发:** 子站 ANUNIT=0x01000064(类型=1, 因子=100)时,当前算成 16777316 × 0.00001 ≈ 167.77 倍率,数据被放大 1.6e4 倍。Lab 子站全 0x00xxxxxx 所以暴露不出来。
- **修复:** `factor = (anunit[i] & 0xFFFFFF) as i32 (符号扩展) * 0.00001`,并暴露类型字节给前端。

---

## P2 — 健壮性 / 可观测性

### [x] 9. FRACSEC 时间质量位丢失
- **位置:** `crates/pmusim-core/src/time_utils.rs:31` + `crates/pmusim-app/src/events.rs:36`
- **现状:** `& 0x00FFFFFF` 砍掉高 8 位,DataInfo 没有 `time_quality: u8` 字段。
- **规约:** §8.11 表 4 16 种时钟状态(锁定/失锁/偏差 10s..0.1s)
- **触发:** 现场 GPS 失锁,运维从 UI 看不出原因。
- **修复:** `time_quality: u8` 字段加到 `DataInfo`,前端按表 4 翻译。

### [x] 10. `mgmt_writer.write_all().await` 在 sessions WRITE 锁内
- **位置:** `crates/pmusim-app/src/network/master.rs:1048-1056`
- **现状:** TCP write 期间整张 session map 被锁,heartbeat / data loop / 新 connect 全排队。
- **触发:** 单个 slow peer(buffer 满 / RTT 高)阻塞所有 session 操作。
- **修复:** 在锁外 take 出 writer 的引用计数副本(或 channel),write 在锁外 await。

### [x] 11. IDCODE 用 `from_utf8_lossy` 损坏非 ASCII 字节
- **位置:** `crates/pmusim-core/src/protocol/parser.rs:18` (`decode_ascii`)
- **触发:** 子站固件 bug 发出 GBK/latin-1 字节(0xC4 0xE3...),decode 得 U+FFFD,re-encode 时 UTF-8 多字节展开成 EF BF BD ... 与原 8 字节完全不同,后续命令子站全拒。
- **修复:** 按字节保存 `Vec<u8>`,显示用 lossy,网络发送用原字节。

### [x] 12. OpenData 不校验 SessionState ≥ Cfg2Sent
- **位置:** `crates/pmusim-app/src/network/master.rs:407`
- **触发:** 用户在 cfg2=None 时点"开启数据",`data_read_loop_outbound` 没 dims,所有帧 parse 失败但 RawFrame 仍上抛,Streaming 状态假成立。
- **修复:** OpenData 入口 guard `state == Cfg2Sent`,否则 emit Error。

### [x] 13. `parse()` 在 SIZE < 2 时下溢索引
- **位置:** `crates/pmusim-core/src/protocol/parser.rs:47`
- **现状:** `read_u16(data, size - 2)`,size=0/1 时 `0usize - 2` release 回绕成 ~usize::MAX → panic。
- **触发:** 网络层 `read_frame` 已 guard `frame_size < 4`,但 lib API 公开,headless / unit test 直接喂坏帧会 panic。
- **修复:** `parse` 头部增 `if size < MIN_FRAME_SIZE_PER_VERSION` 校验。

### [x] 14. `data.len() == size` 不严格相等
- **位置:** `crates/pmusim-core/src/protocol/parser.rs:38`
- **现状:** 只查 `data.len() < size`,尾随字节静默丢弃。
- **触发:** 上层若把粘连帧投入 parse,后续帧丢失。当前网络路径 read_frame 精确切片所以安全,future UDP/file replay 会踩。
- **修复:** 改为 `!=`,或返回消费字节数。

### [x] 15. 心跳响应不校验 SOC echo
- **位置:** `crates/pmusim-app/src/network/master.rs:959`
- **规约:** §8.13 "子站接收到心跳信号后立即将心跳信号返回,与主站下发报文时标相同"
- **现状:** 只匹配 cmd == Heartbeat 字段就清零 missed。
- **触发:** Replay 攻击 / 调试时无法定位时标错位。
- **修复:** session 加 `pending_heartbeat: Option<(soc, fracsec)>`,匹配后才清。

---

## 端到端连接性验证(2026-05-28 跑过)

```
target = 10.15.48.12:8000, V3
1786 DataFrames in 19s ≈ 94 fps
握手 5 步全部成功 (SendCfg1 → SendCfg2Cmd → CFG-2 → SendCfg2 → OpenData)
dup-guard 正常拒绝并行 connect
re-key placeholder "10.15.48.12:8000" → "q1234567" 一次完成
```

**但 1786 帧里 100×7 = 187,200 个采样字段全是 0,STAT=0x0000(子站宣告数据可用)。**
→ 子站 simulator 数据源(IEMP pipeline)没接进 PMU,与 master 无关。
→ Reproducible: `cargo run -p pmusim-app --example headless_smoke -- 10.15.48.12 8000`
