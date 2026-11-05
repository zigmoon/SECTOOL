// Tableau de bord SECTOOL : rendu des vues et interactions.
//
// Le principe : construire le DOM une seule fois par vue, puis mettre à jour
// les textes et les graphiques sur place. Reconstruire à chaque seconde ferait
// perdre les clics et les saisies.

import { api, defaultSettings, isTauri, onSnapshot } from '../lib/api';
import { ago, bps, clock, deviceName, esc } from '../lib/format';
import { icon, KIND_META, type IconName } from '../lib/icons';
import type {
  Device, FirewallRule, Health, HealthIssue, Settings, Snapshot, Theme,
} from '../lib/types';

type ViewId = 'dashboard' | 'map' | 'devices' | 'connections' | 'firewall' | 'log';

const NAV: { id: ViewId; label: string; icon: IconName }[] = [
  { id: 'dashboard', label: 'Tableau de bord', icon: 'dashboard' },
  { id: 'map', label: 'Carte du réseau', icon: 'network' },
  { id: 'devices', label: 'Appareils', icon: 'list' },
  { id: 'connections', label: 'Connexions', icon: 'activity' },
  { id: 'firewall', label: 'Pare-feu', icon: 'shield' },
  { id: 'log', label: 'Journal', icon: 'bell' },
];

const PAGE_META: Record<ViewId, { title: string; sub: string }> = {
  dashboard: { title: 'Tableau de bord', sub: 'Vue d\'ensemble du réseau en temps réel' },
  map: { title: 'Carte du réseau', sub: 'Tous les appareils connectés, cliquez pour agir' },
  devices: { title: 'Appareils', sub: 'Inventaire détaillé du réseau local' },
  connections: { title: 'Connexions', sub: 'Sockets actifs sur cette machine' },
  firewall: { title: 'Pare-feu', sub: 'Règles enregistrées et application au système' },
  log: { title: 'Journal', sub: 'Tout ce qui s\'est passé sur le réseau' },
};

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let snap: Snapshot | null = null;
let rules: FirewallRule[] = [];
let settings: Settings = defaultSettings;
let view: ViewId = 'dashboard';
let selected: string | null = null;
let filterDevices = '';
let filterConns = '';
let scopeFilter: 'all' | 'internet' | 'lan' | 'listen' = 'all';

let dashBuilt = false;
let renderKey = '';
let mapKey = '';
let feedKey = '';
let splitKey = '';
let ifaceKey = '';
let metricValues: Record<string, string> = {};

const kindMeta = (k: string) => KIND_META[k] ?? KIND_META.unknown;

// ───────────────────────── Préférences locales ─────────────────────────────

function readTheme(): Theme {
  const attr = document.documentElement.dataset.theme;
  if (attr === 'light' || attr === 'dark') return attr;
  const saved = localStorage.getItem('sectool-theme');
  if (saved === 'light' || saved === 'dark') return saved;
  return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
}

let theme: Theme = readTheme();
let soundEnabled = localStorage.getItem('sectool-sound') !== '0';

function applyTheme(next: Theme): void {
  theme = next;
  document.documentElement.dataset.theme = next;
  localStorage.setItem('sectool-theme', next);
  syncUiSettings();
  renderTools();
  drawChart();
}

function setSound(on: boolean): void {
  soundEnabled = on;
  localStorage.setItem('sectool-sound', on ? '1' : '0');
  syncUiSettings();
  renderTools();
  if (on) beep('warn');
}

function syncUiSettings(): void {
  settings = {
    ...settings,
    ui: { ...settings.ui, theme, soundEnabled },
  };
  if (isTauri) void api.saveSettings(settings);
}

// ───────────────────────── Son ──────────────────────────────────────────────
// Un bip est synthétisé (aucun fichier audio) pour signaler une alerte.

type AudioCtor = typeof AudioContext;
let audioCtx: AudioContext | null = null;

function audio(): AudioContext | null {
  try {
    if (!audioCtx) {
      const ctor = (window.AudioContext
        ?? (window as unknown as { webkitAudioContext?: AudioCtor }).webkitAudioContext) as AudioCtor | undefined;
      if (!ctor) return null;
      audioCtx = new ctor();
    }
    if (audioCtx.state === 'suspended') void audioCtx.resume();
    return audioCtx;
  } catch {
    return null;
  }
}

function tone(ctx: AudioContext, freq: number, at: number, duration: number, peak = 0.06): void {
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = 'sine';
  osc.frequency.setValueAtTime(freq, ctx.currentTime + at);
  gain.gain.setValueAtTime(0.0001, ctx.currentTime + at);
  gain.gain.exponentialRampToValueAtTime(peak, ctx.currentTime + at + 0.015);
  gain.gain.exponentialRampToValueAtTime(0.0001, ctx.currentTime + at + duration);
  osc.connect(gain).connect(ctx.destination);
  osc.start(ctx.currentTime + at);
  osc.stop(ctx.currentTime + at + duration + 0.02);
}

/** Bip court : deux notes pour un avertissement, trois pour un problème. */
function beep(kind: 'warn' | 'critical'): void {
  if (!soundEnabled) return;
  const ctx = audio();
  if (!ctx) return;
  if (kind === 'critical') {
    tone(ctx, 880, 0, 0.13);
    tone(ctx, 640, 0.16, 0.13);
    tone(ctx, 880, 0.32, 0.18);
  } else {
    tone(ctx, 700, 0, 0.11);
    tone(ctx, 520, 0.14, 0.14);
  }
}

// ───────────────────────── État de santé du réseau ─────────────────────────

function computeHealth(): Health {
  const issues: HealthIssue[] = [];
  if (!snap) {
    return { state: 'ok', label: 'Démarrage', summary: 'En attente du premier relevé', issues };
  }

  const now = Date.now();
  const recent = snap.alerts.filter((a) => now - a.ts < 15 * 60_000);
  const critical = recent.filter((a) => a.level === 'critical');
  const warnings = recent.filter((a) => a.level === 'warn');

  if (critical.length) {
    issues.push({
      level: 'bad',
      title: `${critical.length} alerte${critical.length > 1 ? 's' : ''} critique${critical.length > 1 ? 's' : ''}`,
      detail: `${critical[0].title} — ${critical[0].detail}`,
    });
  }

  const offline = snap.devices.filter((d) => !d.online && !d.isSelf);
  if (offline.length) {
    const names = offline.slice(0, 3).map(deviceName).join(', ');
    issues.push({
      level: 'warn',
      title: `${offline.length} appareil${offline.length > 1 ? 's' : ''} hors ligne`,
      detail: `${names}${offline.length > 3 ? '…' : ''} ne répond${offline.length > 1 ? 'ent' : ''} plus au balayage.`,
    });
  }

  const gateway = snap.devices.find((d) => d.isGateway);
  if (snap.gateway && !gateway) {
    issues.push({
      level: 'warn',
      title: 'Passerelle non identifiée',
      detail: `La box ${snap.gateway} répond au système mais n'apparaît pas encore dans le balayage.`,
    });
  } else if (gateway && !gateway.online) {
    issues.push({
      level: 'bad',
      title: 'Passerelle injoignable',
      detail: `La box ${snap.gateway} ne répond plus : Internet peut être coupé.`,
    });
  }

  const activeRules = rules.filter((r) => r.enabled).length;
  if (activeRules > 0 && !snap.firewall.appliedAt) {
    issues.push({
      level: 'warn',
      title: `${activeRules} règle${activeRules > 1 ? 's' : ''} de pare-feu non appliquée${activeRules > 1 ? 's' : ''}`,
      detail: 'Les règles sont enregistrées mais le pare-feu du système n\'a pas encore été modifié.',
    });
  }
  if (activeRules > 0 && snap.firewall.readOnly) {
    issues.push({
      level: 'warn',
      title: 'Pare-feu en lecture seule',
      detail: 'SECTOOL n\'a pas les droits administrateur : les règles ne peuvent pas être appliquées.',
    });
  }

  if (snap.devices.length === 0 && now - (snap.lastScanAt ?? 0) > 20_000) {
    issues.push({
      level: 'bad',
      title: 'Aucun appareil détecté',
      detail: 'Vérifiez que cette machine est bien connectée au réseau local (Wi-Fi ou câble).',
    });
  }

  if (!issues.length && warnings.length) {
    issues.push({
      level: 'warn',
      title: `${warnings.length} avertissement${warnings.length > 1 ? 's' : ''} récent${warnings.length > 1 ? 's' : ''}`,
      detail: warnings[0].detail,
    });
  }

  const state: Health['state'] = issues.some((i) => i.level === 'bad')
    ? 'bad'
    : issues.length
      ? 'warn'
      : 'ok';

  const online = snap.devices.filter((d) => d.online).length;
  const label = state === 'bad' ? 'Problème détecté' : state === 'warn' ? 'À vérifier' : 'Tout va bien';
  const summary = state === 'ok'
    ? `${online} appareil${online > 1 ? 's' : ''} en ligne · aucune alerte`
    : `${issues.length} point${issues.length > 1 ? 's' : ''} à examiner`;

  return { state, label, summary, issues };
}

// ───────────────────────── Navigation et en-tête ───────────────────────────

function renderNav(): void {
  const nav = $('nav');
  if (!nav.dataset.built) {
    nav.innerHTML = NAV.map((n) =>
      `<button class="nav-item" type="button" data-view="${n.id}">${icon(n.icon, 18)}<span>${n.label}</span><span class="count" hidden></span></button>`).join('');
    nav.dataset.built = '1';
  }
  const st = snap?.stats;
  NAV.forEach((n, index) => {
    const btn = nav.children[index] as HTMLElement;
    btn.classList.toggle('on', view === n.id);
    const badge = btn.querySelector('.count') as HTMLElement;
    let text = '';
    let alert = false;
    if (st) {
      if (n.id === 'devices') text = `${st.devicesOnline}/${st.devices}`;
      if (n.id === 'connections') text = String(st.connections);
      if (n.id === 'firewall') text = String(rules.filter((r) => r.enabled).length);
      if (n.id === 'log' && st.alerts) {
        text = String(st.alerts);
        alert = st.alertsCritical > 0;
      }
    }
    if (badge.textContent !== text) badge.textContent = text;
    if (badge.hidden !== (text === '')) badge.hidden = text === '';
    badge.classList.toggle('alert', alert);
  });
}

let toolsKey = '';

function renderTools(): void {
  // Comme pour le menu : ne toucher au DOM que si l'état a changé, sinon un
  // clic peut tomber pendant le remplacement du contenu du bouton.
  const key = `${theme}|${soundEnabled}|${!!document.fullscreenElement}`;
  if (toolsKey === key) return;
  toolsKey = key;

  const themeBtn = $('btn-theme');
  themeBtn.innerHTML = icon(theme === 'dark' ? 'sun' : 'moon', 17);
  themeBtn.title = theme === 'dark' ? 'Passer en thème clair' : 'Passer en thème sombre';
  themeBtn.classList.toggle('on', theme === 'light');

  const soundBtn = $('btn-sound');
  soundBtn.innerHTML = icon(soundEnabled ? 'sound' : 'mute', 17);
  soundBtn.title = soundEnabled ? 'Alertes sonores activées' : 'Alertes sonores coupées';
  soundBtn.classList.toggle('on', soundEnabled);

  const fsBtn = $('btn-fullscreen');
  fsBtn.innerHTML = icon(document.fullscreenElement ? 'exitFullscreen' : 'fullscreen', 17);
  fsBtn.title = document.fullscreenElement ? 'Quitter le plein écran (F11)' : 'Plein écran (F11)';
  fsBtn.classList.toggle('on', !!document.fullscreenElement);
}

function renderTopbar(): void {
  if (!snap) return;
  const ver = `v${snap.version}`;
  if ($('ver').textContent !== ver) $('ver').textContent = ver;
  const host = `${snap.hostname} · ${snap.localIp ?? '—'}`;
  if ($('host').textContent !== host) $('host').textContent = host;

  const fw = snap.firewall;
  const pill = $('fw-pill');
  const text = fw.readOnly ? 'Lecture seule' : 'Administrateur';
  if (pill.dataset.key !== text) {
    pill.dataset.key = text;
    pill.className = `pill ${fw.readOnly ? 'warn' : 'ok'}`;
    pill.innerHTML = `<span class="dot"></span>${icon(fw.readOnly ? 'shieldAlert' : 'shieldOk', 15)}<span>${text}</span>`;
  }

  const scanBtn = $('btn-scan');
  if (!scanBtn.dataset.ready) {
    scanBtn.innerHTML = icon('refresh', 16);
    scanBtn.dataset.ready = '1';
  }
  scanBtn.classList.toggle('spin', snap.scanning);
}

function updateDocumentTitle(): void {
  const n = snap?.stats.alerts ?? 0;
  const critical = (snap?.stats.alertsCritical ?? 0) > 0;
  document.title = n > 0 ? `${critical ? '⚠ ' : ''}(${n}) SECTOOL — Réseau` : 'SECTOOL — Réseau & Pare-feu';
}

// ───────────────────────── Graphique de débit ──────────────────────────────

function cssVar(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value || fallback;
}

function withAlpha(color: string, alpha: number): string {
  const c = color.trim();
  if (c.startsWith('#')) {
    const hex = c.slice(1);
    const full = hex.length === 3 ? hex.split('').map((x) => x + x).join('') : hex;
    const n = Number.parseInt(full.slice(0, 6), 16);
    if (Number.isNaN(n)) return c;
    return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
  }
  return c;
}

function drawChart(): void {
  const c = document.getElementById('chart') as HTMLCanvasElement | null;
  if (!c || !snap || view !== 'dashboard') return;
  const rect = c.getBoundingClientRect();
  const w = Math.max(140, Math.floor(rect.width));
  const h = Math.max(90, Math.floor(rect.height));
  const dpr = window.devicePixelRatio || 1;
  if (c.width !== Math.floor(w * dpr) || c.height !== Math.floor(h * dpr)) {
    c.width = Math.floor(w * dpr);
    c.height = Math.floor(h * dpr);
  }
  const g = c.getContext('2d');
  if (!g) return;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);

  const grid = cssVar('--grid-line', 'rgba(255,255,255,.05)');
  const colRx = cssVar('--rx', '#4de3c1');
  const colTx = cssVar('--tx', '#8b8cff');

  g.strokeStyle = grid;
  g.lineWidth = 1;
  for (let i = 1; i <= 3; i += 1) {
    const y = (h / 4) * i;
    g.beginPath();
    g.moveTo(0, y);
    g.lineTo(w, y);
    g.stroke();
  }

  const pts = snap.traffic.history.slice(-120);
  if (pts.length < 2) return;
  const max = Math.max(40_000, ...pts.map((p) => Math.max(p.rxBps, p.txBps))) * 1.15;
  const n = pts.length;
  const px = (i: number) => (i / (n - 1)) * w;
  const py = (v: number) => h - (v / max) * (h - 12) - 5;

  const draw = (pick: (p: typeof pts[0]) => number, color: string) => {
    g.beginPath();
    g.moveTo(px(0), py(pick(pts[0])));
    for (let i = 1; i < n - 1; i += 1) {
      const xc = (px(i) + px(i + 1)) / 2;
      const yc = (py(pick(pts[i])) + py(pick(pts[i + 1]))) / 2;
      g.quadraticCurveTo(px(i), py(pick(pts[i])), xc, yc);
    }
    g.lineTo(px(n - 1), py(pick(pts[n - 1])));

    const fill = g.createLinearGradient(0, 0, 0, h);
    fill.addColorStop(0, withAlpha(color, 0.26));
    fill.addColorStop(1, withAlpha(color, 0));
    g.save();
    g.lineTo(w, h);
    g.lineTo(0, h);
    g.closePath();
    g.fillStyle = fill;
    g.fill();
    g.restore();

    g.beginPath();
    g.moveTo(px(0), py(pick(pts[0])));
    for (let i = 1; i < n - 1; i += 1) {
      const xc = (px(i) + px(i + 1)) / 2;
      const yc = (py(pick(pts[i])) + py(pick(pts[i + 1]))) / 2;
      g.quadraticCurveTo(px(i), py(pick(pts[i])), xc, yc);
    }
    g.lineTo(px(n - 1), py(pick(pts[n - 1])));
    g.strokeStyle = color;
    g.lineWidth = 2;
    g.lineJoin = 'round';
    g.stroke();

    g.beginPath();
    g.arc(px(n - 1), py(pick(pts[n - 1])), 3, 0, Math.PI * 2);
    g.fillStyle = color;
    g.fill();
  };

  draw((p) => p.txBps, colTx);
  draw((p) => p.rxBps, colRx);
}

// ───────────────────────── Vue 1 : tableau de bord ─────────────────────────

const METRICS: { key: string; icon: IconName; label: string }[] = [
  { key: 'devices', icon: 'wifi', label: 'Appareils en ligne' },
  { key: 'internet', icon: 'globe', label: 'Connexions Internet' },
  { key: 'listen', icon: 'radio', label: 'Ports en écoute' },
  { key: 'blocked', icon: 'ban', label: 'Appareils bloqués' },
  { key: 'rules', icon: 'shield', label: 'Règles actives' },
  { key: 'gateway', icon: 'timer', label: 'Latence box' },
];

function buildDashboard(): void {
  dashBuilt = true;
  renderKey = 'dashboard';
  $('view').innerHTML = `<div class="dash">
    <div class="dash-hero">
      <div class="card status-card">
        <button class="orb" type="button" id="orb" data-health="ok" data-health-open aria-label="État du réseau">
          <span class="orb-ring"></span>
          <span class="orb-glow"></span>
          <span class="orb-core">
            <span class="orb-icon" id="orb-icon"></span>
            <span class="orb-title" id="orb-title">Analyse…</span>
            <span class="orb-sub" id="orb-sub">Premier relevé en cours</span>
          </span>
        </button>
        <div class="status-reasons" id="orb-chips"></div>
        <div class="status-hint">Cliquez pour savoir pourquoi ${icon('chevron', 13)}</div>
      </div>

      <div class="card traffic-card">
        <div class="card-title">Trafic réseau <span class="right" id="traffic-when"></span></div>
        <div class="traffic-head">
          <div class="tt rx">
            <span class="label">${icon('down', 12)} Réception</span>
            <span class="value" id="val-rx">—</span>
          </div>
          <div class="tt tx">
            <span class="label">${icon('up', 12)} Émission</span>
            <span class="value" id="val-tx">—</span>
          </div>
          <div class="legend-inline">
            <span><i style="background:var(--rx)"></i>RX</span>
            <span><i style="background:var(--tx)"></i>TX</span>
          </div>
        </div>
        <div class="traffic-canvas-wrap"><canvas id="chart" aria-label="Débit réseau en temps réel"></canvas></div>
      </div>
    </div>

    <div class="metric-grid" id="metrics">
      ${METRICS.map((m) => `<div class="metric" data-metric="${m.key}">
        <div class="top">${icon(m.icon, 14)}<span>${m.label}</span></div>
        <b>—</b><span class="sub">&nbsp;</span>
      </div>`).join('')}
    </div>

    <div class="dash-bottom">
      <div class="card">
        <div class="card-title">Alertes récentes
          <span class="right"><button class="btn ghost" type="button" data-view="log">Tout voir ${icon('chevron', 13)}</button></span>
        </div>
        <div class="feed" id="feed"></div>
      </div>
      <div class="card">
        <div class="card-title">Répartition des appareils</div>
        <div class="split-bars" id="split"></div>
      </div>
      <div class="card">
        <div class="card-title">Interfaces réseau</div>
        <div class="iface-list" id="ifaces"></div>
      </div>
    </div>
  </div>`;
  feedKey = '';
  splitKey = '';
  ifaceKey = '';
  metricValues = {};
}

function setMetric(key: string, value: string, sub: string, tone?: 'good' | 'bad'): void {
  const card = document.querySelector(`[data-metric="${key}"]`) as HTMLElement | null;
  if (!card) return;
  const b = card.querySelector('b') as HTMLElement;
  const s = card.querySelector('.sub') as HTMLElement;
  if (metricValues[key] !== value) {
    metricValues[key] = value;
    b.textContent = value;
    card.classList.remove('changed');
    void card.offsetWidth;
    card.classList.add('changed');
  }
  if (s.textContent !== sub) s.textContent = sub;
  card.classList.toggle('good', tone === 'good');
  card.classList.toggle('bad', tone === 'bad');
}

function renderDashboard(): void {
  if (!snap) return;
  if (!dashBuilt || !document.getElementById('orb')) buildDashboard();

  const health = computeHealth();
  const st = snap.stats;

  const orb = $('orb');
  orb.dataset.health = health.state;
  const orbIcon = health.state === 'bad' ? 'shieldAlert' : health.state === 'warn' ? 'warn' : 'shieldOk';
  const iconHtml = icon(orbIcon, health.state === 'ok' ? 34 : 32);
  const orbIconEl = $('orb-icon');
  if (orbIconEl.dataset.key !== iconHtml) {
    orbIconEl.dataset.key = iconHtml;
    orbIconEl.innerHTML = iconHtml;
  }
  $('orb-title').textContent = health.label;
  $('orb-sub').textContent = health.summary;

  const chips = health.issues.slice(0, 2)
    .map((i) => `<span class="tag ${i.level === 'bad' ? 'bad' : 'warn'}">${esc(i.title)}</span>`).join('');
  const chipsEl = $('orb-chips');
  if (chipsEl.dataset.key !== chips) {
    chipsEl.dataset.key = chips;
    chipsEl.innerHTML = chips;
  }

  $('val-rx').textContent = bps(snap.traffic.rxBps);
  $('val-tx').textContent = bps(snap.traffic.txBps);
  $('traffic-when').textContent = snap.lastScanAt ? `balayage ${ago(snap.lastScanAt)}` : '';

  const gateway = snap.devices.find((d) => d.isGateway);
  const rtt = gateway?.rttMs;
  setMetric('devices', `${st.devicesOnline}/${st.devices}`, 'sur le réseau local', st.devicesOnline > 0 ? 'good' : 'bad');
  setMetric('internet', String(st.connectionsInternet), `${st.connections} socket${st.connections > 1 ? 's' : ''} au total`);
  setMetric('listen', String(st.listening), 'ouverts sur cette machine');
  setMetric('blocked', String(st.blockedDevices), st.blockedDevices ? 'isolés par une règle' : 'aucun blocage actif', st.blockedDevices ? 'bad' : undefined);
  setMetric('rules', String(st.activeRules), st.activeRules ? 'règles de pare-feu' : 'pare-feu non modifié');
  setMetric('gateway', rtt != null ? `${rtt} ms` : '—', snap.gateway ?? 'passerelle inconnue', rtt != null && rtt < 30 ? 'good' : undefined);

  renderFeed();
  renderSplit();
  renderIfaces();
  drawChart();
}

function renderFeed(): void {
  const feed = document.getElementById('feed');
  if (!feed || !snap) return;
  const list = snap.alerts.slice(0, 5);
  const key = list.map((a) => a.id).join(',');
  if (feedKey === key) return;
  feedKey = key;
  if (!list.length) {
    feed.innerHTML = '<div class="empty">Aucun événement pour le moment. Le réseau est calme.</div>';
    return;
  }
  const ic: Record<string, IconName> = { info: 'info', warn: 'warn', critical: 'shieldAlert' };
  feed.innerHTML = list.map((a) => `<div class="feed-item ${a.level}">
    <span class="dotwrap">${icon(ic[a.level] ?? 'info', 15)}</span>
    <div><b>${esc(a.title)}</b><p>${esc(a.detail)}</p></div>
    <time>${clock(a.ts)}</time>
  </div>`).join('');
}

function renderSplit(): void {
  const el = document.getElementById('split');
  if (!el || !snap) return;
  const counts = new Map<string, number>();
  for (const d of snap.devices) {
    counts.set(d.kind, (counts.get(d.kind) ?? 0) + 1);
  }
  const rows = [...counts.entries()].sort((a, b) => b[1] - a[1]).slice(0, 7);
  const key = rows.map(([k, v]) => `${k}:${v}`).join('|');
  if (splitKey === key) return;
  splitKey = key;
  const max = Math.max(1, ...rows.map(([, v]) => v));
  const total = snap.devices.length || 1;
  el.innerHTML = rows.map(([kind, count]) => {
    const meta = kindMeta(kind);
    const pct = Math.round((count / max) * 100);
    return `<div class="split-row">
      <span class="name">${icon(kind as IconName, 13)}${esc(meta.label)}</span>
      <span class="bar"><i style="width:${pct}%;background:${meta.color}"></i></span>
      <span class="n">${count}/${total}</span>
    </div>`;
  }).join('') || '<div class="empty">Pas encore d\'appareil.</div>';
}

function renderIfaces(): void {
  const el = document.getElementById('ifaces');
  if (!el || !snap) return;
  const list = (snap.traffic.interfaces ?? []).filter((i) => i.rxBps > 0 || i.txBps > 0).slice(0, 6);
  const key = list.map((i) => `${i.name}:${i.rxBps}:${i.txBps}`).join('|');
  if (ifaceKey === key) return;
  ifaceKey = key;
  el.innerHTML = list.length
    ? list.map((i) => `<div class="iface-row">
        <span class="name">${esc(i.name)}</span>
        <span class="v rx">↓ ${bps(i.rxBps)}</span>
        <span class="v tx">↑ ${bps(i.txBps)}</span>
      </div>`).join('')
    : '<div class="empty">Aucune interface active.</div>';
}

// ───────────────────────── Vue 2 : carte ────────────────────────────────────

function nodeHtml(d: Device, x: number, y: number, hub: boolean): string {
  const m = kindMeta(d.kind);
  const cls = ['node', d.online ? 'on' : 'off', d.blocked ? 'blocked' : '', hub ? 'hub' : '', selected === d.id ? 'sel' : ''].join(' ');
  return `<button class="${cls}" type="button" data-device="${esc(d.id)}" style="left:${x}%;top:${y}%;--c:${m.color}" title="${esc(deviceName(d))}">
    <span class="ico">${icon(d.kind as IconName, hub ? 30 : 25)}</span><span class="dot"></span>
    <span class="lbl">${esc(deviceName(d))}</span><span class="sub">${esc(d.ip ?? '')}</span></button>`;
}

function renderMap(): void {
  if (!snap) return;
  const devs = snap.devices;
  const hubDev = devs.find((d) => d.isGateway) ?? null;
  const others = devs.filter((d) => d !== hubDev);
  const detail = devs.find((d) => d.id === selected);
  const sig = JSON.stringify([others.map((d) => [d.id, d.online, d.blocked, d.hostname, d.kind]), hubDev?.id, selected]);
  const key = `map|${sig}|${JSON.stringify(detail ?? null)}`;
  if (mapKey === key) return;
  mapKey = key;

  const online = others.filter((d) => d.online);
  const offline = others.filter((d) => !d.online);
  const place = (list: Device[], r: number, off: number) => list.map((d, i) => {
    const a = (i / Math.max(1, list.length)) * Math.PI * 2 - Math.PI / 2 + off;
    return { d, x: 50 + Math.cos(a) * r * 0.9, y: 50 + Math.sin(a) * r };
  });
  const all = [...place(online, 33, 0), ...place(offline, 43, 0.42)];
  const links = all.map(({ d, x, y }) =>
    `<line x1="50%" y1="50%" x2="${x}%" y2="${y}%" class="${!d.online ? 'off' : d.blocked ? 'blk' : ''}"/>`).join('');
  const hubNode = hubDev
    ? nodeHtml(hubDev, 50, 50, true)
    : `<div class="node hub on" style="left:50%;top:50%;--c:var(--accent)"><span class="ico">${icon('router', 30)}</span><span class="lbl">Réseau</span></div>`;

  $('view').innerHTML = `<div class="map-wrap">
    <div class="map-col">
      <div class="map" id="map"><svg class="links">${links}</svg>${hubNode}${all.map(({ d, x, y }) => nodeHtml(d, x, y, false)).join('')}</div>
      <div class="legend">
        <span><i style="background:var(--ok)"></i>En ligne</span>
        <span><i style="background:var(--faint)"></i>Hors ligne</span>
        <span><i style="background:var(--bad)"></i>Bloqué</span>
      </div>
    </div>
    <aside class="card detail" id="detail"></aside>
  </div>`;
  $('detail').innerHTML = detailHtml(detail);
}

function detailHtml(d?: Device): string {
  if (!d) {
    return `<div class="card-title">Détails</div>
      <div class="empty">Cliquez sur un appareil de la carte pour voir ses informations et le bloquer si besoin.</div>
      ${snap ? `<dl class="kv">
        <dt>Cette machine</dt><dd>${esc(snap.hostname)}</dd>
        <dt>Adresse</dt><dd class="mono">${esc(snap.localIp ?? '—')}</dd>
        <dt>Passerelle</dt><dd class="mono">${esc(snap.gateway ?? '—')}</dd>
        <dt>Dernier balayage</dt><dd>${snap.lastScanMs ?? '—'} ms · ${ago(snap.lastScanAt)}</dd>
      </dl>` : ''}`;
  }
  const m = kindMeta(d.kind);
  const ports = (d.ports ?? []).map((p) => `<span class="chip">${p.port}${p.service ? ' · ' + esc(p.service) : ''}</span>`).join('')
    || '<span class="muted tiny">aucun port détecté</span>';
  const canBlock = !!d.ip && !d.isSelf && !d.isGateway;
  return `<div class="head" style="--c:${m.color}"><span class="ico">${icon(d.kind as IconName, 24)}</span>
      <div><h3>${esc(deviceName(d))}</h3><span class="muted tiny">${m.label}${d.isGateway ? ' · passerelle' : ''}${d.isSelf ? ' · cette machine' : ''}</span></div></div>
    <dl class="kv">
      <dt>Adresse IP</dt><dd class="mono">${esc(d.ip ?? '—')}</dd>
      <dt>Adresse MAC</dt><dd class="mono">${esc(d.mac ?? '—')}</dd>
      <dt>Constructeur</dt><dd>${esc(d.vendor ?? 'inconnu')}</dd>
      ${d.model ? `<dt>Modèle</dt><dd>${esc(d.model)}</dd>` : ''}
      <dt>Statut</dt><dd>${d.online ? '<span class="tag ok">en ligne</span>' : '<span class="tag">hors ligne</span>'}${d.blocked ? ' <span class="tag bad">bloqué</span>' : ''}</dd>
      <dt>Latence</dt><dd>${d.rttMs != null ? d.rttMs + ' ms' : '—'}</dd>
      <dt>Dernière vue</dt><dd>${ago(d.lastSeen)}</dd>
    </dl>
    <div class="card-title">Ports ouverts</div><div class="chips">${ports}</div>
    ${canBlock ? `<p><button class="btn ${d.blocked ? '' : 'danger'}" type="button" data-block="${esc(d.id)}" data-to="${d.blocked ? 0 : 1}">${icon(d.blocked ? 'check' : 'ban', 16)}${d.blocked ? 'Débloquer cet appareil' : 'Bloquer cet appareil'}</button></p>
    <p class="muted tiny">La règle est enregistrée ici ; elle ne touche le pare-feu du système qu'après « Appliquer » dans l'onglet Pare-feu.</p>` : ''}`;
}

// ───────────────────────── Vue 3 : appareils ────────────────────────────────

function renderDevices(): void {
  if (!snap) return;
  const q = filterDevices.toLowerCase();
  const list = snap.devices.filter((d) => !q || [d.hostname, d.ip, d.mac, d.vendor, d.kind].join(' ').toLowerCase().includes(q));
  const key = `devices|${q}|${JSON.stringify(list.map((d) => [d.id, d.online, d.blocked, d.rttMs]))}`;
  if (renderKey === key) return;
  const focus = document.activeElement?.id === 'q-dev';
  renderKey = key;
  const rows = list.map((d) => {
    const m = kindMeta(d.kind);
    return `<tr>
      <td><span class="cell-ico" style="--c:${m.color}"><span class="ico">${icon(d.kind as IconName, 16)}</span>
        <span><b>${esc(deviceName(d))}</b><br><span class="muted tiny">${m.label}</span></span></span></td>
      <td class="mono">${esc(d.ip ?? '—')}</td>
      <td class="mono">${esc(d.mac ?? '—')}</td>
      <td>${esc(d.vendor ?? '—')}</td>
      <td>${(d.ports ?? []).slice(0, 4).map((p) => `<span class="chip">${p.port}</span>`).join(' ') || '—'}</td>
      <td class="mono">${d.rttMs != null ? d.rttMs + ' ms' : '—'}</td>
      <td>${d.blocked ? '<span class="tag bad">bloqué</span>' : d.online ? '<span class="tag ok">en ligne</span>' : `<span class="tag">${ago(d.lastSeen)}</span>`}</td>
    </tr>`;
  }).join('');
  $('view').innerHTML = `<div class="toolbar">
      <input id="q-dev" type="search" placeholder="Rechercher (nom, IP, MAC, marque…)" value="${esc(filterDevices)}">
      <button class="btn" type="button" data-export>${icon('download', 15)}Exporter en JSON</button>
    </div>
    <div class="table-wrap"><table>
      <thead><tr><th>Appareil</th><th>IP</th><th>MAC</th><th>Constructeur</th><th>Ports</th><th>Latence</th><th>Statut</th></tr></thead>
      <tbody>${rows || '<tr><td colspan="7" class="empty">Aucun appareil.</td></tr>'}</tbody>
    </table></div>`;
  if (focus) {
    const i = $('q-dev') as HTMLInputElement;
    i.focus();
    i.setSelectionRange(i.value.length, i.value.length);
  }
}

// ───────────────────────── Vue 4 : connexions ───────────────────────────────

function renderConnections(): void {
  if (!snap) return;
  const q = filterConns.toLowerCase();
  const list = snap.connections.filter((c) => {
    if (scopeFilter === 'internet' && c.scope !== 'internet') return false;
    if (scopeFilter === 'lan' && c.scope !== 'lan') return false;
    if (scopeFilter === 'listen' && c.state !== 'listen') return false;
    if (scopeFilter === 'all' && c.state !== 'established' && c.state !== 'listen' && c.state !== 'udp') return false;
    return !q || [c.process, c.remoteAddr, c.localPort, c.remotePort].join(' ').toLowerCase().includes(q);
  }).slice(0, 400);
  const key = `conns|${q}|${scopeFilter}|${list.map((c) => c.id).join(',')}`;
  if (renderKey === key) return;
  const focus = document.activeElement?.id === 'q-con';
  renderKey = key;
  const tabs: [typeof scopeFilter, string][] = [
    ['all', 'Actives'], ['internet', 'Internet'], ['lan', 'Réseau local'], ['listen', 'En écoute'],
  ];
  const rows = list.map((c) => `<tr>
    <td><b>${esc(c.process ?? '—')}</b><br><span class="muted tiny">PID ${c.pid ?? '—'}</span></td>
    <td><span class="tag">${c.proto.toUpperCase()}</span></td>
    <td class="mono">${esc(c.localAddr)}:${c.localPort}</td>
    <td class="mono">${c.remotePort ? esc(c.remoteAddr) + ':' + c.remotePort : esc(c.remoteAddr)}</td>
    <td><span class="tag ${c.scope === 'internet' ? 'net' : ''}">${{ internet: 'Internet', lan: 'Réseau local', local: 'Local', multicast: 'Multicast' }[c.scope]}</span></td>
    <td>${c.state === 'listen' ? '<span class="tag warn">en écoute</span>' : esc(c.state)}</td>
  </tr>`).join('');
  $('view').innerHTML = `<div class="toolbar">
      ${tabs.map(([id, label]) => `<button class="btn ${scopeFilter === id ? 'primary' : ''}" type="button" data-scope="${id}">${label}</button>`).join('')}
      <input id="q-con" type="search" placeholder="Filtrer (programme, adresse, port…)" value="${esc(filterConns)}">
    </div>
    <div class="table-wrap"><table>
      <thead><tr><th>Programme</th><th>Proto</th><th>Local</th><th>Distant</th><th>Zone</th><th>État</th></tr></thead>
      <tbody>${rows || '<tr><td colspan="6" class="empty">Aucune connexion.</td></tr>'}</tbody>
    </table></div>`;
  if (focus) {
    const i = $('q-con') as HTMLInputElement;
    i.focus();
    i.setSelectionRange(i.value.length, i.value.length);
  }
}

// ───────────────────────── Vue 5 : pare-feu ─────────────────────────────────

let fwBuilt = false;

function renderFirewall(): void {
  if (!snap) return;
  if (!fwBuilt || !document.getElementById('fw-list')) {
    fwBuilt = true;
    renderKey = '';
    $('view').innerHTML = `<div class="dash">
      <div class="card" id="fw-status">
        <div class="row" style="display:flex;align-items:center;gap:14px;flex-wrap:wrap">
          <span id="fw-icon"></span>
          <div style="flex:1;min-width:220px">
            <b id="fw-title"></b> — moteur : <span class="mono" id="fw-engine"></span><br>
            <span class="muted" id="fw-msg"></span><br>
            <span class="muted tiny" id="fw-when"></span>
          </div>
          <button class="btn" type="button" data-preview></button>
          <button class="btn primary" type="button" data-apply></button>
          <button class="btn danger" type="button" data-flush></button>
        </div>
      </div>
      <div class="card">
        <div class="card-title">Nouvelle règle</div>
        <form class="form" id="rule-form">
          <label class="wide">Nom<input name="name" required placeholder="ex. Bloquer le partage Windows"></label>
          <label>Action<select name="action"><option value="block">Bloquer</option><option value="allow">Autoriser</option></select></label>
          <label>Sens<select name="direction"><option value="in">Entrant</option><option value="out">Sortant</option><option value="both">Les deux</option></select></label>
          <label>Protocole<select name="protocol"><option value="tcp">TCP</option><option value="udp">UDP</option><option value="any">Tous</option></select></label>
          <label>Adresse ou réseau<input name="remote" placeholder="192.168.1.42 ou 10.0.0.0/8"></label>
          <label>Ports locaux<input name="localPorts" placeholder="445,139 ou 8000-8100"></label>
          <label>Programme (Windows)<input name="app" placeholder="C:\\Chemin\\app.exe"></label>
          <div><button class="btn primary" type="submit"></button></div>
        </form>
      </div>
      <div class="card">
        <div class="card-title">Règles (<span id="fw-count">0</span>)</div>
        <div id="fw-list"></div>
      </div>
    </div>`;
    const form = $('rule-form') as HTMLFormElement;
    (form.querySelector('button[type="submit"]') as HTMLElement).innerHTML = `${icon('plus', 16)}Ajouter`;
    (document.querySelector('[data-preview]') as HTMLElement).innerHTML = `${icon('eye', 16)}Aperçu des commandes`;
    (document.querySelector('[data-apply]') as HTMLElement).innerHTML = `${icon('play', 16)}Appliquer`;
    (document.querySelector('[data-flush]') as HTMLElement).innerHTML = `${icon('eraser', 16)}Tout retirer`;
  }
  const fw = snap.firewall;
  const setText = (id: string, text: string) => {
    const el = document.getElementById(id);
    if (el && el.textContent !== text) el.textContent = text;
  };
  $('fw-icon').innerHTML = `<span style="color:${fw.readOnly ? 'var(--warn)' : 'var(--ok)'}">${icon(fw.readOnly ? 'shieldAlert' : 'shieldOk', 32)}</span>`;
  setText('fw-title', fw.readOnly ? 'Lecture seule' : 'Prêt à appliquer');
  setText('fw-engine', fw.backend);
  setText('fw-msg', fw.message);
  setText('fw-when', fw.appliedAt ? `Dernière application ${ago(fw.appliedAt)}` : '');

  const key = `fwlist|${JSON.stringify(rules)}`;
  const listEl = $('fw-list');
  if (listEl.dataset.key === key) return;
  listEl.dataset.key = key;
  $('fw-count').textContent = String(rules.length);
  listEl.innerHTML = rules.length
    ? `<div class="table-wrap"><table>
        <thead><tr><th>Actif</th><th>Règle</th><th>Action</th><th>Sens</th><th>Cible</th><th></th></tr></thead>
        <tbody>${rules.map((r) => `<tr>
          <td><input type="checkbox" data-toggle="${esc(r.id)}" ${r.enabled ? 'checked' : ''} aria-label="Activer"></td>
          <td><b>${esc(r.name)}</b>${r.auto ? ' <span class="tag net">auto</span>' : ''}<br><span class="muted tiny">${esc(r.note ?? '')}</span></td>
          <td><span class="tag ${r.action === 'block' ? 'bad' : 'ok'}">${r.action === 'block' ? 'Bloquer' : 'Autoriser'}</span></td>
          <td>${{ in: 'entrant', out: 'sortant', both: '2 sens' }[r.direction]} · ${r.protocol.toUpperCase()}</td>
          <td class="mono">${esc([r.remote, r.localPorts && 'port ' + r.localPorts, r.app].filter(Boolean).join(' · ') || 'tout')}</td>
          <td><button class="btn ghost" type="button" data-del="${esc(r.id)}" title="Supprimer">${icon('trash', 15)}</button></td>
        </tr>`).join('')}</tbody>
      </table></div>`
    : '<div class="empty">Aucune règle. Ajoutez-en une ci-dessus, ou bloquez un appareil depuis la carte.</div>';
}

// ───────────────────────── Vue 6 : journal ──────────────────────────────────

function renderLog(): void {
  if (!snap) return;
  const key = `log|${snap.alerts.map((a) => a.id).join(',')}`;
  if (renderKey === key) return;
  renderKey = key;
  const ic: Record<string, IconName> = { info: 'info', warn: 'warn', critical: 'shieldAlert' };
  $('view').innerHTML = `<div class="toolbar">
      <button class="btn" type="button" data-clear>${icon('trash', 15)}Vider le journal</button>
    </div>
    <div class="table-wrap">${snap.alerts.length
      ? snap.alerts.map((a) => `<div class="alert ${a.level}">
          <span class="lv">${icon(ic[a.level] ?? 'info', 17)}</span>
          <div><b>${esc(a.title)}</b><br><span class="muted">${esc(a.detail)}</span></div>
          <time>${clock(a.ts)}</time>
        </div>`).join('')
      : '<div class="empty">Rien à signaler.</div>'}</div>`;
}

// ───────────────────────── Orchestration ────────────────────────────────────

function render(): void {
  renderNav();
  renderTopbar();
  renderTools();
  switch (view) {
    case 'dashboard': renderDashboard(); break;
    case 'map': renderMap(); break;
    case 'devices': renderDevices(); break;
    case 'connections': renderConnections(); break;
    case 'firewall': renderFirewall(); break;
    case 'log': renderLog(); break;
  }
}

function setView(v: ViewId): void {
  if (view === v) return;
  view = v;
  renderKey = '';
  mapKey = '';
  dashBuilt = false;
  fwBuilt = false;
  selected = v === 'map' ? selected : null;
  $('page-title').textContent = PAGE_META[v].title;
  $('page-sub').textContent = PAGE_META[v].sub;
  render();
}

let toastTimer = 0;
function toast(msg: string, err = false): void {
  const t = $('toast');
  t.textContent = msg;
  t.className = `toast ${err ? 'err' : ''}`;
  t.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => { t.hidden = true; }, 5000);
}

async function refreshRules(list?: FirewallRule[]): Promise<void> {
  rules = list ?? await api.rules();
  render();
}

// ───────────────────────── Dialogues ────────────────────────────────────────

function openDialog(html: string): void {
  const d = $('dialog') as HTMLDialogElement;
  d.innerHTML = html;
  if (!d.open) d.showModal();
}

function showHealth(): void {
  const health = computeHealth();
  const rows = health.issues.length
    ? health.issues.map((i) => `<div class="reason ${i.level}">
        <span class="ic">${icon(i.level === 'bad' ? 'shieldAlert' : 'warn', 15)}</span>
        <div><b>${esc(i.title)}</b><p>${esc(i.detail)}</p></div>
      </div>`).join('')
    : `<div class="reason ok">
        <span class="ic">${icon('check', 15)}</span>
        <div><b>Aucun problème détecté</b>
        <p>${snap ? `${snap.stats.devicesOnline} appareil(s) en ligne, aucun appareil hors ligne, passerelle joignable et aucune alerte critique sur les 15 dernières minutes.` : 'Le premier balayage est en cours.'}</p></div>
      </div>`;

  const counts = snap ? `
    <div class="reasons">
      <div class="reason ${snap.stats.devicesOnline ? 'ok' : 'warn'}"><span class="ic">${icon('wifi', 15)}</span>
        <div><b>${snap.stats.devicesOnline} appareil(s) en ligne</b><p>sur ${snap.stats.devices} connus du réseau local.</p></div></div>
      <div class="reason ${snap.stats.alertsCritical ? 'bad' : 'ok'}"><span class="ic">${icon('bell', 15)}</span>
        <div><b>${snap.stats.alerts} événement(s) au journal</b><p>${snap.stats.alertsCritical} critique(s).</p></div></div>
      <div class="reason ${snap.firewall.readOnly ? 'warn' : 'ok'}"><span class="ic">${icon('shield', 15)}</span>
        <div><b>Pare-feu : ${snap.firewall.readOnly ? 'lecture seule' : 'administrateur'}</b><p>${esc(snap.firewall.message)}</p></div></div>
    </div>` : '';

  openDialog(`<h2>État du réseau</h2>
    <p class="muted">${esc(health.label)} — ${esc(health.summary)}</p>
    <div class="reasons">${rows}</div>
    ${health.issues.length ? counts : ''}
    <div class="dialog-actions">
      <button class="btn" type="button" data-view="log" data-close-after>${icon('bell', 15)}Voir le journal</button>
      <button class="btn primary" type="button" data-close>Fermer</button>
    </div>`);
}

async function showPreview(): Promise<void> {
  const p = await api.preview();
  openDialog(`<h2>Aperçu — ${esc(p.backend)}</h2>
    <p class="muted">${p.ruleCount} règle(s) active(s). Voici exactement ce qui sera exécuté :</p>
    ${p.warnings.map((w) => `<p class="tag warn" style="display:block;border-radius:8px;white-space:normal">${esc(w)}</p>`).join('')}
    <pre class="cmds">${esc(p.commands.join('\n') || '(rien à exécuter)')}</pre>
    <div class="dialog-actions"><button class="btn" type="button" data-close>Fermer</button></div>`);
}

async function runAction(kind: 'apply' | 'flush'): Promise<void> {
  const label = kind === 'apply'
    ? 'Appliquer les règles au pare-feu du système ?'
    : 'Retirer toutes les règles SECTOOL du pare-feu du système ?';
  if (!window.confirm(label)) return;
  toast('En cours… une autorisation administrateur peut être demandée.');
  try {
    const r = kind === 'apply' ? await api.apply() : await api.flush();
    toast(r.message, !r.ok);
    await refreshRules(r.rules);
  } catch (e) {
    toast(String(e), true);
  }
}

async function toggleFullscreen(): Promise<void> {
  try {
    if (document.fullscreenElement) await document.exitFullscreen();
    else await document.documentElement.requestFullscreen();
  } catch {
    toast('Le plein écran n\'est pas disponible ici.', true);
  }
}

// ───────────────────────── Événements ──────────────────────────────────────

document.addEventListener('click', async (ev) => {
  const el = (ev.target as HTMLElement).closest<HTMLElement>(
    '[data-view],[data-device],[data-block],[data-scope],[data-preview],[data-apply],[data-flush],[data-del],[data-clear],[data-export],[data-close],[data-close-after],[data-health-open]',
  );
  if (!el) return;
  const ds = el.dataset;
  try {
    if (ds.healthOpen !== undefined) showHealth();
    else if (ds.view) {
      if (el.closest('dialog')) ($('dialog') as HTMLDialogElement).close();
      setView(ds.view as ViewId);
    } else if (ds.device) {
      selected = ds.device;
      mapKey = '';
      render();
    } else if (ds.block) {
      const d = snap?.devices.find((x) => x.id === ds.block);
      if (d?.ip) {
        await refreshRules(await api.setDeviceBlocked(d.ip, deviceName(d), ds.to === '1'));
        toast(ds.to === '1' ? 'Règle de blocage créée (à appliquer dans Pare-feu).' : 'Règle retirée.');
      }
    } else if (ds.scope) {
      scopeFilter = ds.scope as typeof scopeFilter;
      renderKey = '';
      render();
    } else if (el.hasAttribute('data-preview')) await showPreview();
    else if (el.hasAttribute('data-apply')) await runAction('apply');
    else if (el.hasAttribute('data-flush')) await runAction('flush');
    else if (ds.del) await refreshRules(await api.deleteRule(ds.del));
    else if (el.hasAttribute('data-clear')) {
      await api.clearAlerts();
      if (snap) snap.alerts = [];
      renderKey = '';
      render();
    } else if (el.hasAttribute('data-close')) ($('dialog') as HTMLDialogElement).close();
    else if (el.hasAttribute('data-export')) {
      const blob = new Blob([await api.exportReport()], { type: 'application/json' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = 'sectool-rapport.json';
      a.click();
      URL.revokeObjectURL(a.href);
    }
  } catch (e) {
    toast(String(e), true);
  }
});

document.addEventListener('input', (ev) => {
  const t = ev.target as HTMLInputElement;
  if (t.id === 'q-dev') {
    filterDevices = t.value;
    renderKey = '';
    renderDevices();
  }
  if (t.id === 'q-con') {
    filterConns = t.value;
    renderKey = '';
    renderConnections();
  }
});

document.addEventListener('change', async (ev) => {
  const t = ev.target as HTMLInputElement;
  if (t.dataset.toggle) await refreshRules(await api.toggleRule(t.dataset.toggle, t.checked));
});

document.addEventListener('submit', async (ev) => {
  const f = ev.target as HTMLFormElement;
  if (f.id !== 'rule-form') return;
  ev.preventDefault();
  const v = Object.fromEntries(new FormData(f).entries()) as Record<string, string>;
  try {
    await refreshRules(await api.saveRule({
      name: v.name,
      action: v.action as 'block' | 'allow',
      direction: v.direction as 'in' | 'out' | 'both',
      protocol: v.protocol as 'tcp' | 'udp' | 'any',
      remote: v.remote,
      localPorts: v.localPorts,
      app: v.app,
      enabled: true,
    }));
    f.reset();
    toast('Règle ajoutée.');
  } catch (e) {
    toast(String(e), true);
  }
});

$('btn-scan').addEventListener('click', () => {
  audio(); // débloque l'audio au premier clic
  void api.scanNow();
  toast('Balayage lancé.');
});
$('btn-theme').addEventListener('click', () => applyTheme(theme === 'dark' ? 'light' : 'dark'));
$('btn-sound').addEventListener('click', () => setSound(!soundEnabled));
$('btn-fullscreen').addEventListener('click', () => void toggleFullscreen());

document.addEventListener('fullscreenchange', renderTools);
document.addEventListener('keydown', (ev) => {
  if (ev.key === 'F11') {
    ev.preventDefault();
    void toggleFullscreen();
  }
  if (ev.key === 'Escape' && document.fullscreenElement) void document.exitFullscreen();
});
window.addEventListener('resize', drawChart);

// ───────────────────────── Démarrage ───────────────────────────────────────

let firstSnapshot = true;
const seenAlerts = new Set<string>();

function absorbAlerts(s: Snapshot): void {
  const incoming = s.alerts ?? [];
  if (firstSnapshot) {
    // Au démarrage, on marque tout comme déjà vu : pas de bip pour l'historique.
    incoming.forEach((a) => seenAlerts.add(a.id));
    firstSnapshot = false;
    return;
  }
  const fresh = incoming.filter((a) => !seenAlerts.has(a.id));
  fresh.forEach((a) => seenAlerts.add(a.id));
  const loud = fresh.filter((a) => a.level !== 'info');
  if (loud.length) {
    beep(loud.some((a) => a.level === 'critical') ? 'critical' : 'warn');
  }
}

if (!isTauri) $('demo-banner').hidden = false;
renderTools();
$('page-title').textContent = PAGE_META[view].title;
$('page-sub').textContent = PAGE_META[view].sub;

void (async () => {
  rules = await api.rules();
  settings = await api.settings();
  if (localStorage.getItem('sectool-sound') === null) {
    soundEnabled = settings.ui.soundEnabled;
  }
  renderTools();
  await onSnapshot((s) => {
    absorbAlerts(s);
    snap = s;
    render();
    updateDocumentTitle();
  });
})();
