// Passerelle vers Rust (Tauri) ou vers les données de démonstration.

import type {
  ActionResult, Alert, ApplyPlan, FirewallRule, RuleInput, Settings, Snapshot,
} from './types';
import { demoPlan, demoRules, demoSnapshot } from './demo';

export const isTauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;

type Unlisten = () => void;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core');
  return invoke<T>(cmd, args);
}

// État de la démo (règles modifiables en mémoire).
let rulesDemo: FirewallRule[] = [...demoRules];

export const defaultSettings: Settings = {
  scanIntervalSecs: 5,
  probeTimeoutMs: 350,
  probePorts: [22, 53, 80, 443, 445, 554, 631, 1883, 3389, 5000, 5001, 8008, 8009, 8080, 9100, 32400, 62078],
  enableMdns: true,
  enableReverseDns: true,
  enableArpSweep: true,
  maxHosts: 512,
  alertNewOutbound: false,
  ui: { reduceMotion: false, showIpv6: false, showMap: true, theme: 'dark', soundEnabled: true },
};

export async function onSnapshot(cb: (s: Snapshot) => void): Promise<Unlisten> {
  if (isTauri) {
    const { listen } = await import('@tauri-apps/api/event');
    const stop = await listen<Snapshot>('sectool://snapshot', (e) => cb(e.payload));
    try { cb(await invoke<Snapshot>('get_snapshot')); } catch { /* le premier événement suivra */ }
    return stop;
  }
  cb(demoSnapshot(rulesDemo));
  const timer = window.setInterval(() => cb(demoSnapshot(rulesDemo)), 1000);
  return () => window.clearInterval(timer);
}

export const api = {
  scanNow: () => (isTauri ? invoke<void>('scan_now') : Promise.resolve()),
  rules: () => (isTauri ? invoke<FirewallRule[]>('get_rules') : Promise.resolve(rulesDemo)),

  saveRule: async (rule: RuleInput): Promise<FirewallRule[]> => {
    if (isTauri) return invoke('save_rule', { rule });
    const id = rule.id ?? `r${Date.now()}`;
    const entry: FirewallRule = { ...rule, id, auto: !!rule.auto, created: Date.now() };
    rulesDemo = rulesDemo.some((r) => r.id === id)
      ? rulesDemo.map((r) => (r.id === id ? { ...r, ...entry } : r))
      : [...rulesDemo, entry];
    return rulesDemo;
  },

  deleteRule: async (id: string): Promise<FirewallRule[]> => {
    if (isTauri) return invoke('delete_rule', { id });
    rulesDemo = rulesDemo.filter((r) => r.id !== id);
    return rulesDemo;
  },

  toggleRule: async (id: string, enabled: boolean): Promise<FirewallRule[]> => {
    if (isTauri) return invoke('toggle_rule', { id, enabled });
    rulesDemo = rulesDemo.map((r) => (r.id === id ? { ...r, enabled } : r));
    return rulesDemo;
  },

  setDeviceBlocked: async (ip: string, label: string, blocked: boolean): Promise<FirewallRule[]> => {
    if (isTauri) return invoke('set_device_blocked', { ip, label, blocked });
    rulesDemo = rulesDemo.filter((r) => !(r.auto && r.remote === ip));
    if (blocked) {
      rulesDemo = [...rulesDemo, {
        id: `d${Date.now()}`, name: `Bloquer ${label}`, action: 'block', direction: 'both', protocol: 'any',
        remote: ip, enabled: true, auto: true, created: Date.now(), note: 'Créé depuis la carte du réseau',
      }];
    }
    return rulesDemo;
  },

  preview: (): Promise<ApplyPlan> => (isTauri ? invoke('preview_rules') : Promise.resolve(demoPlan(rulesDemo))),

  apply: async (): Promise<ActionResult> => {
    if (isTauri) return invoke('apply_rules');
    return {
      ok: false, message: 'Mode démonstration : aucune règle n\'est appliquée au système.',
      log: '', rules: rulesDemo, at: Date.now(),
      status: demoSnapshot(rulesDemo).firewall,
    };
  },

  flush: async (): Promise<ActionResult> => {
    if (isTauri) return invoke('flush_rules');
    return {
      ok: false, message: 'Mode démonstration : rien à retirer.', log: '', rules: rulesDemo, at: Date.now(),
      status: demoSnapshot(rulesDemo).firewall,
    };
  },

  clearAlerts: () => (isTauri ? invoke<void>('clear_alerts') : Promise.resolve()),

  settings: async (): Promise<Settings> => {
    if (!isTauri) return defaultSettings;
    try { return await invoke<Settings>('get_settings'); } catch { return defaultSettings; }
  },

  saveSettings: async (settings: Settings): Promise<void> => {
    if (!isTauri) return;
    try { await invoke('save_settings_cmd', { settings }); } catch { /* préférence locale seulement */ }
  },

  exportReport: async (): Promise<string> => {
    if (isTauri) return invoke('export_report');
    return JSON.stringify({ demo: true, snapshot: demoSnapshot(rulesDemo), rules: rulesDemo }, null, 2);
  },
};

export type { Alert };
