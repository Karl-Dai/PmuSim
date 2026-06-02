export interface SubDataInfo {
  soc: number; fracsec: number; stat: number;
  freq: number; dfreq: number;
  phasors: [number, number][];
  analog: number[]; digital: number[];
}

export type SubEvent =
  | { type: "Listening"; mgmt_port: number; data_port: number }
  | { type: "MasterConnected"; peer_ip: string }
  | { type: "MasterDisconnected"; peer_ip: string }
  | { type: "CommandReceived"; cmd: number; name: string }
  | { type: "Cfg1Sent" }
  | { type: "Cfg2Sent" }
  | { type: "Cfg2Received" }
  | { type: "Cfg2Rejected"; reason: string }
  | { type: "StreamingStarted" }
  | { type: "StreamingStopped" }
  | { type: "DataFrameSent"; data: SubDataInfo }
  | { type: "RawFrame"; direction: string; hex: string }
  | { type: "Error"; error: string };

export interface PhasorInput { magnitude: number; phase_deg: number }
export interface ConfigInput {
  protocol: "V2" | "V3";
  idcode: string; stn: string;
  mgmt_port: number; data_port: number;
  data_rate_fps: number;
  phasors: PhasorInput[];
  analogs: number[]; digitals: number[];
}
