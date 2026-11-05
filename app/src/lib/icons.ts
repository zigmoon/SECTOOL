// Icônes Lucide (ISC) intégrées en SVG. Seules celles utilisées sont embarquées.

import {
  createElement, Router, Laptop, Monitor, Smartphone, Tablet, Tv, Printer, Camera,
  Speaker, Gamepad2, HardDrive, Cpu, Server, CircleHelp, Activity, Shield, ShieldCheck,
  ShieldAlert, ShieldOff, Network, List, Bell, Settings, RefreshCw, Download, Plus,
  Trash2, Ban, CircleCheck, ArrowDown, ArrowUp, Globe, Wifi, X, TriangleAlert, Info,
  Play, Eraser, Search, Eye, LayoutDashboard, Sun, Moon, Maximize, Minimize, Volume2,
  VolumeX, Zap, ChevronRight, Signal, Timer, Radio,
} from 'lucide';

const ICONS = {
  router: Router, computer: Monitor, laptop: Laptop, phone: Smartphone, tablet: Tablet,
  tv: Tv, printer: Printer, camera: Camera, speaker: Speaker, console: Gamepad2,
  nas: HardDrive, iot: Cpu, server: Server, unknown: CircleHelp,
  activity: Activity, shield: Shield, shieldOk: ShieldCheck, shieldAlert: ShieldAlert,
  shieldOff: ShieldOff, network: Network, list: List, bell: Bell, settings: Settings,
  refresh: RefreshCw, download: Download, plus: Plus, trash: Trash2, ban: Ban,
  check: CircleCheck, down: ArrowDown, up: ArrowUp, globe: Globe, wifi: Wifi, close: X,
  warn: TriangleAlert, info: Info, play: Play, eraser: Eraser, search: Search, eye: Eye,
  dashboard: LayoutDashboard, sun: Sun, moon: Moon, fullscreen: Maximize,
  exitFullscreen: Minimize, sound: Volume2, mute: VolumeX, zap: Zap,
  chevron: ChevronRight, signal: Signal, timer: Timer, radio: Radio,
} as const;

export type IconName = keyof typeof ICONS;

/** Renvoie le balisage SVG d'une icône. */
export function icon(name: IconName, size = 20, stroke = 1.75): string {
  const el = createElement(ICONS[name] ?? CircleHelp);
  el.setAttribute('width', String(size));
  el.setAttribute('height', String(size));
  el.setAttribute('stroke-width', String(stroke));
  el.setAttribute('aria-hidden', 'true');
  return el.outerHTML;
}

/** Libellé français et couleur d'un type d'appareil. */
export const KIND_META: Record<string, { label: string; color: string }> = {
  router: { label: 'Box / routeur', color: '#38bdf8' },
  computer: { label: 'Ordinateur', color: '#a78bfa' },
  laptop: { label: 'Ordinateur portable', color: '#a78bfa' },
  phone: { label: 'Téléphone', color: '#34d399' },
  tablet: { label: 'Tablette', color: '#34d399' },
  tv: { label: 'Télévision', color: '#fb923c' },
  printer: { label: 'Imprimante', color: '#f472b6' },
  camera: { label: 'Caméra', color: '#f87171' },
  speaker: { label: 'Enceinte', color: '#fbbf24' },
  console: { label: 'Console de jeu', color: '#c084fc' },
  nas: { label: 'Stockage réseau', color: '#2dd4bf' },
  iot: { label: 'Objet connecté', color: '#facc15' },
  server: { label: 'Serveur', color: '#60a5fa' },
  unknown: { label: 'Appareil inconnu', color: '#94a3b8' },
};
