export interface SessionInfo {
  idcode: string;
  peerIp: string;
  state: "connecting" | "connected" | "cfg1_received" | "cfg2_sent" | "streaming" | "disconnected";
  /** 拨号目标 `${host}:${mgmtPort}`,跨 re-key 稳定,用于按目标重连。 */
  dialKey?: string;
}

export interface ConfigInfo {
  cfgType: number;
  version: number;
  stn: string;
  idcode: string;
  formatFlags: number;
  period: number;
  measRate: number;
  phnmr: number;
  annmr: number;
  dgnmr: number;
  channelNames: string[];
  anunit: number[];
}

export interface DataInfo {
  soc: number;
  fracsec: number;
  stat: number;
  /** FORMAT bits 0-3 from the matching CFG-2. Bit0=1 polar phasors, bit1-3 = float toggles. */
  format_flags: number;
  /** FRACSEC bit27-24 = §8.11 表 4 GPS time-quality code. 0=lock, 0xF=invalid. */
  time_quality: number;
  freq: number;
  dfreq: number;
  analog: number[];
  digital: number[];
  /** Each pair: (real, imag) when format bit0=0, (magnitude, angle) when bit0=1. */
  phasors: [number, number][];
  /** 后端接收时刻 now − 报文时间戳(ms)。正=报文滞后本地，负=超前。 */
  local_offset_ms: number;
}

export interface RawFrameInfo {
  idcode: string;
  direction: "send" | "recv";
  hex: string;
}

export type PmuEvent =
  | { type: "SessionCreated"; idcode: string; peer_ip: string }
  | { type: "SessionDisconnected"; idcode: string }
  | { type: "Cfg1Received"; idcode: string; cfg: ConfigInfo }
  | { type: "Cfg2Sent"; idcode: string }
  | { type: "Cfg2Skipped"; idcode: string }
  | { type: "Cfg2Received"; idcode: string; cfg: ConfigInfo }
  | { type: "StreamingStarted"; idcode: string }
  | { type: "StreamingStopped"; idcode: string }
  | { type: "DataFrame"; idcode: string; data: DataInfo }
  | { type: "RawFrame"; idcode: string; direction: string; hex: string }
  | { type: "HeartbeatTimeout"; idcode: string }
  | { type: "TimestampAnomaly"; idcode: string; kind: string; expected_ms: number; actual_ms: number; soc: number; fracsec: number; frame_time: string }
  | { type: "Error"; idcode: string; error: string };

export interface AnomalyEntry {
  id: number;
  localTime: string; // 收报墙钟时刻 "HH:MM:SS"
  idcode: string;
  kind: string; // "backward" | "gap" | "stall" | 未知 code 原样
  expectedMs: number;
  actualMs: number; // 回退时为负
  soc: number;
  fracsec: number;
  frameTime: string; // 后端给的北京时间
}
