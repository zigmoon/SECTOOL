// Données de démonstration : permettent de voir l'interface dans un navigateur
// (npm run dev) sans l'application de bureau. Clairement signalées dans l'UI.

import type { ApplyPlan, Device, FirewallRule, Snapshot, Connection, TrafficPoint } from './types';

const now = () => Date.now();

function dev(p: Partial<Device> & { id: string }): Device {
  return {
    kind: 'unknown', online: true, isGateway: false, isSelf: false,
    firstSeen: now() - 86_400_000 * 3, lastSeen: now(), sources: ['arp'], blocked: false, ...p,
  };
}

export const demoDevices: Device[] = [
  dev({ id: 'd1', ip: '192.168.1.1', mac: '6c:4c:bc:be:91:b7', hostname: 'Freebox Server', vendor: 'Free', kind: 'router', isGateway: true, rttMs: 2, ports: [{ port: 80, service: 'HTTP' }, { port: 443, service: 'HTTPS' }] }),
  dev({ id: 'd2', ip: '192.168.1.4', mac: 'a4:83:e7:11:22:33', hostname: 'Bureau-PC', vendor: 'Intel', kind: 'computer', isSelf: true, rttMs: 0 }),
  dev({ id: 'd3', ip: '192.168.1.3', mac: '00:11:32:68:9e:c5', hostname: 'DiskStation', vendor: 'Synology', kind: 'nas', rttMs: 3, ports: [{ port: 445, service: 'SMB' }, { port: 5000, service: 'UPnP' }] }),
  dev({ id: 'd4', ip: '192.168.1.8', mac: '10:b1:df:a3:19:b3', hostname: 'Imprimante-Salon', vendor: 'Canon', kind: 'printer', rttMs: 9, ports: [{ port: 9100, service: 'Impression' }, { port: 631, service: 'IPP' }] }),
  dev({ id: 'd5', ip: '192.168.1.21', mac: 'f0:18:98:5a:10:44', hostname: 'iPhone-de-Marie', vendor: 'Apple', kind: 'phone', rttMs: 14 }),
  dev({ id: 'd6', ip: '192.168.1.22', mac: '34:23:ba:77:ab:09', hostname: 'Galaxy-S24', vendor: 'Samsung', kind: 'phone', rttMs: 21 }),
  dev({ id: 'd7', ip: '192.168.1.30', mac: '3c:5a:b4:00:a1:55', hostname: 'Chromecast-TV', vendor: 'Google', kind: 'tv', rttMs: 11, ports: [{ port: 8009, service: 'Cast' }] }),
  dev({ id: 'd8', ip: '192.168.1.41', mac: '24:0a:c4:9c:33:12', hostname: 'prise-cuisine', vendor: 'Espressif', kind: 'iot', rttMs: 31 }),
  dev({ id: 'd9', ip: '192.168.1.50', mac: '44:19:b6:cc:01:90', hostname: 'camera-entree', vendor: 'Hikvision', kind: 'camera', rttMs: 17, ports: [{ port: 554, service: 'RTSP' }] }),
  dev({ id: 'd10', ip: '192.168.1.62', mac: '5c:aa:fd:12:34:ab', hostname: 'Sonos-Chambre', vendor: 'Sonos', kind: 'speaker', rttMs: 12 }),
  dev({ id: 'd11', ip: '192.168.1.70', mac: 'dc:a6:32:00:11:22', hostname: 'raspberrypi', vendor: 'Raspberry Pi', kind: 'server', rttMs: 6, ports: [{ port: 22, service: 'SSH' }] }),
  dev({ id: 'd12', ip: '192.168.1.88', mac: '9e:ff:2d:62:86:e0', kind: 'unknown', online: false, lastSeen: now() - 3_600_000 }),
];

export const demoRules: FirewallRule[] = [
  { id: 'r1', name: 'Bloquer Telnet entrant', action: 'block', direction: 'in', protocol: 'tcp', localPorts: '23', enabled: true, auto: false, created: now() - 86_400_000 },
  { id: 'r2', name: 'Bloquer SMB depuis Internet', action: 'block', direction: 'in', protocol: 'tcp', localPorts: '445,139', remote: '0.0.0.0/0', enabled: false, auto: false, created: now() - 3_600_000 },
];

const procs = ['chrome.exe', 'firefox.exe', 'Teams.exe', 'Spotify.exe', 'OneDrive.exe', 'svchost.exe', 'steam.exe'];
const remotes = ['142.250.74.206', '151.101.1.69', '104.18.32.47', '52.97.201.34', '35.186.224.25', '192.168.1.3', '192.168.1.1'];

export function demoConnections(): Connection[] {
  return remotes.map((r, i) => ({
    id: `c${i}`, proto: 'tcp', localAddr: '192.168.1.4', localPort: 50000 + i * 37,
    remoteAddr: r, remotePort: r.startsWith('192.') ? 445 : 443, state: 'established',
    pid: 1000 + i * 111, process: procs[i % procs.length], scope: r.startsWith('192.') ? 'lan' : 'internet',
  }));
}

let tick = 0;
const hist: TrafficPoint[] = [];

export function demoSnapshot(rules: FirewallRule[]): Snapshot {
  tick++;
  const rx = Math.max(0, 800_000 + Math.sin(tick / 4) * 600_000 + Math.random() * 300_000);
  const tx = Math.max(0, 150_000 + Math.cos(tick / 5) * 100_000 + Math.random() * 80_000);
  hist.push({ ts: now(), rxBps: rx, txBps: tx });
  while (hist.length > 180) hist.shift();
  const blockedIps = new Set(rules.filter((r) => r.enabled && r.action === 'block').map((r) => r.remote));
  const devices = demoDevices.map((d) => ({ ...d, blocked: !!d.ip && blockedIps.has(d.ip) }));
  const connections = demoConnections();
  return {
    ts: now(), version: '0.2.0', platform: 'windows', hostname: 'Bureau-PC', isElevated: false, scanning: false,
    lastScanMs: 1840, lastScanAt: now() - 2000, gateway: '192.168.1.1', localIp: '192.168.1.4',
    interfaces: [{ name: 'Ethernet', mac: 'a4:83:e7:11:22:33', ipv4: ['192.168.1.4'], subnet: '192.168.1.0/24', isUp: true, isDefault: true }],
    devices, connections,
    alerts: [
      { id: 'a1', ts: now() - 120_000, level: 'info', kind: 'new_device', title: 'Nouvel appareil détecté', detail: 'Galaxy-S24 — 192.168.1.22' },
      { id: 'a2', ts: now() - 900_000, level: 'warn', kind: 'device_offline', title: 'Appareil hors ligne', detail: '192.168.1.88 ne répond plus.' },
    ],
    firewall: { supported: true, elevated: false, backend: 'windows', activeRules: rules.filter((r) => r.enabled).length, message: 'Mode lecture seule : lancez SECTOOL en administrateur pour appliquer les règles.', readOnly: true },
    traffic: {
      rxBps: rx, txBps: tx, history: [...hist],
      interfaces: [
        { name: 'Ethernet', rxBps: rx, txBps: tx, rxTotal: 0, txTotal: 0 },
        { name: 'Wi-Fi', rxBps: Math.round(rx * 0.18), txBps: Math.round(tx * 0.22), rxTotal: 0, txTotal: 0 },
        { name: 'Loopback', rxBps: 1200, txBps: 1200, rxTotal: 0, txTotal: 0 },
      ],
    },
    stats: {
      devices: devices.length, devicesOnline: devices.filter((d) => d.online).length,
      connections: connections.length, connectionsInternet: connections.filter((c) => c.scope === 'internet').length,
      listening: 14, alerts: 2, alertsCritical: 0, blockedDevices: devices.filter((d) => d.blocked).length,
      activeRules: rules.filter((r) => r.enabled).length,
    },
  };
}

export function demoPlan(rules: FirewallRule[]): ApplyPlan {
  return {
    backend: 'Pare-feu Windows (netsh)', elevated: false, ruleCount: rules.filter((r) => r.enabled).length,
    warnings: ['Une autorisation administrateur sera demandée pour modifier le pare-feu du système.'],
    commands: rules.filter((r) => r.enabled).map((r) => `netsh advfirewall firewall add rule name="SECTOOL|${r.id}|${r.name}" dir=in action=${r.action} protocol=TCP`),
  };
}
