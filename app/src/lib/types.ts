// Types alignés sur les structures Rust (serde camelCase).

export type DeviceKind =
  | 'router' | 'computer' | 'laptop' | 'phone' | 'tablet' | 'tv' | 'printer'
  | 'camera' | 'speaker' | 'console' | 'nas' | 'iot' | 'server' | 'unknown';

export interface PortInfo { port: number; service?: string; rttMs?: number }

export interface Device {
  id: string;
  mac?: string;
  ip?: string;
  hostname?: string;
  vendor?: string;
  model?: string;
  kind: DeviceKind;
  online: boolean;
  isGateway: boolean;
  isSelf: boolean;
  firstSeen: number;
  lastSeen: number;
  rttMs?: number;
  ports?: PortInfo[];
  services?: string[];
  sources: string[];
  blocked: boolean;
  note?: string;
}

export type Scope = 'local' | 'lan' | 'internet' | 'multicast';

export interface Connection {
  id: string;
  proto: string;
  localAddr: string;
  localPort: number;
  remoteAddr: string;
  remotePort: number;
  state: string;
  pid?: number;
  process?: string;
  scope: Scope;
}

export interface Alert {
  id: string;
  ts: number;
  level: 'info' | 'warn' | 'critical';
  kind: string;
  title: string;
  detail: string;
  deviceId?: string;
}

export interface FirewallRule {
  id: string;
  name: string;
  action: 'allow' | 'block';
  direction: 'in' | 'out' | 'both';
  protocol: 'tcp' | 'udp' | 'any';
  remote?: string;
  localPorts?: string;
  remotePorts?: string;
  app?: string;
  note?: string;
  enabled: boolean;
  auto: boolean;
  created: number;
  lastApplied?: number;
}

export interface FirewallStatus {
  supported: boolean;
  elevated: boolean;
  backend: string;
  activeRules: number;
  appliedAt?: number;
  message: string;
  readOnly: boolean;
}

export interface TrafficPoint { ts: number; rxBps: number; txBps: number }

export interface InterfaceInfo {
  name: string;
  mac?: string;
  ipv4: string[];
  subnet?: string;
  isUp: boolean;
  isDefault: boolean;
}

export interface Snapshot {
  ts: number;
  version: string;
  platform: string;
  hostname: string;
  isElevated: boolean;
  scanning: boolean;
  lastScanMs?: number;
  lastScanAt?: number;
  gateway?: string;
  localIp?: string;
  interfaces: InterfaceInfo[];
  devices: Device[];
  connections: Connection[];
  alerts: Alert[];
  firewall: FirewallStatus;
  traffic: {
    rxBps: number;
    txBps: number;
    history: TrafficPoint[];
    interfaces?: { name: string; rxBps: number; txBps: number; rxTotal: number; txTotal: number }[];
  };
  stats: {
    devices: number; devicesOnline: number; connections: number;
    connectionsInternet: number; listening: number; alerts: number;
    alertsCritical: number; blockedDevices: number; activeRules: number;
  };
}

export interface ApplyPlan {
  backend: string;
  elevated: boolean;
  commands: string[];
  warnings: string[];
  ruleCount: number;
}

export interface ActionResult {
  ok: boolean;
  message: string;
  log: string;
  rules: FirewallRule[];
  status: FirewallStatus;
  at: number;
}

export interface RuleInput {
  id?: string;
  name: string;
  action: 'allow' | 'block';
  direction: 'in' | 'out' | 'both';
  protocol: 'tcp' | 'udp' | 'any';
  remote?: string;
  localPorts?: string;
  remotePorts?: string;
  app?: string;
  note?: string;
  enabled: boolean;
  auto?: boolean;
}

export type Theme = 'dark' | 'light';

export interface UiSettings {
  reduceMotion: boolean;
  showIpv6: boolean;
  showMap: boolean;
  theme: Theme;
  soundEnabled: boolean;
}

export interface Settings {
  scanIntervalSecs: number;
  probeTimeoutMs: number;
  probePorts: number[];
  enableMdns: boolean;
  enableReverseDns: boolean;
  enableArpSweep: boolean;
  maxHosts: number;
  alertNewOutbound: boolean;
  subnetOverride?: string;
  ui: UiSettings;
}

/** Un motif expliquant l'état du réseau, affiché sous l'orbe de statut. */
export interface HealthIssue {
  level: 'bad' | 'warn';
  title: string;
  detail: string;
}

export interface Health {
  state: 'ok' | 'warn' | 'bad';
  label: string;
  summary: string;
  issues: HealthIssue[];
}
