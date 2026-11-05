// Tableau de bord SECTOOL : rendu des vues et interactions.

import { api, isTauri, onSnapshot } from '../lib/api';
import { ago, bps, clock, deviceName, esc } from '../lib/format';
import { icon, KIND_META, type IconName } from '../lib/icons';
import type { Device, FirewallRule, Snapshot } from '../lib/types';

type ViewId = 'map' | 'devices' | 'connections' | 'firewall' | 'log';

const NAV: { id: ViewId; label: string; icon: IconName }[] = [
  { id: 'map', label: 'Carte du réseau', icon: 'network' },
  { id: 'devices', label: 'Appareils', icon: 'list' },
  { id: 'connections', label: 'Connexions', icon: 'activity' },
  { id: 'firewall', label: 'Pare-feu', icon: 'shield' },
  { id: 'log', label: 'Journal', icon: 'bell' },
];

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

let snap: Snapshot | null = null;
let rules: FirewallRule[] = [];
let view: ViewId = 'map';
let selected: string | null = null;
let filterDevices = '';
let filterConns = '';
let scopeFilter: 'all' | 'internet' | 'lan' | 'listen' = 'all';
let renderKey = '';
let mapKey = '';

const kindMeta = (k: string) => KIND_META[k] ?? KIND_META.unknown;

// ───────────────────────── En-tête ─────────────────────────────────

const STAT_TILES: { icon: IconName; label: string }[] = [
  { icon: 'wifi', label: 'appareils en ligne' },
  { icon: 'globe', label: 'connexions Internet' },
  { icon: 'eye', label: 'ports en écoute' },
  { icon: 'shieldOk', label: 'appareils bloqués' },
];

function renderNav(): void {
  const nav = $('nav');
  if (!nav.dataset.built) {
    nav.innerHTML = NAV.map((n) =>
      `<button class="nav-item" data-view="${n.id}">${icon(n.icon, 18)}<span>${n.label}</span><span class="count" hidden></span></button>`).join('');
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

function renderTop(): void {
  if (!snap) return;
  const st = snap.stats;
  const stats = $('stats');
  if (!stats.dataset.built) {
    stats.innerHTML = STAT_TILES.map((t) =>
      `<div class="stat"><b>0</b><span>${icon(t.icon, 15)}${t.label}</span></div>`).join('');
    stats.dataset.built = '1';
  }
  const values = [st.devicesOnline, st.connectionsInternet, st.listening, st.blockedDevices];
  values.forEach((value, index) => {
    const cell = stats.children[index]?.querySelector('b');
    if (cell && cell.textContent !== String(value)) cell.textContent = String(value);
  });

  $('rx').innerHTML = `${icon('down', 16)} ${bps(snap.traffic.rxBps)}`;
  $('tx').innerHTML = `${icon('up', 16)} ${bps(snap.traffic.txBps)}`;
  $('ver').textContent = `v${snap.version}`;
  $('host').textContent = `${snap.hostname} · ${snap.localIp ?? '—'}`;
  const scanBtn = $('btn-scan');
  if (!scanBtn.dataset.ready) {
    scanBtn.innerHTML = icon('refresh', 16);
    scanBtn.dataset.ready = '1';
  }
  scanBtn.classList.toggle('spin', snap.scanning);

  const fw = snap.firewall;
  const pill = $('fw-pill');
  const pillHtml = `${icon(fw.readOnly ? 'shieldAlert' : 'shieldOk', 16)}<span>${fw.readOnly ? 'Lecture seule' : 'Administrateur'}</span>`;
  if (pill.dataset.key !== pillHtml) {
    pill.dataset.key = pillHtml;
    pill.className = `pill ${fw.readOnly ? 'warn' : 'ok'}`;
    pill.innerHTML = pillHtml;
  }
  drawChart();
}

function drawChart(): void {
  if (!snap) return;
  const c = $('chart') as unknown as HTMLCanvasElement;
  const dpr = window.devicePixelRatio || 1;
  const w = c.clientWidth, h = c.clientHeight;
  if (c.width !== w * dpr) { c.width = w * dpr; c.height = h * dpr; }
  const g = c.getContext('2d')!;
  g.setTransform(dpr, 0, 0, dpr, 0, 0);
  g.clearRect(0, 0, w, h);
  const pts = snap.traffic.history;
  if (pts.length < 2) return;
  const max = Math.max(50_000, ...pts.map((p) => Math.max(p.rxBps, p.txBps))) * 1.15;
  const line = (sel: (p: typeof pts[0]) => number, color: string, fill: string) => {
    g.beginPath();
    pts.forEach((p, i) => {
      const x = (i / (pts.length - 1)) * w;
      const y = h - (sel(p) / max) * (h - 6) - 2;
      i ? g.lineTo(x, y) : g.moveTo(x, y);
    });
    g.strokeStyle = color; g.lineWidth = 1.8; g.stroke();
    g.lineTo(w, h); g.lineTo(0, h); g.closePath(); g.fillStyle = fill; g.fill();
  };
  line((p) => p.rxBps, '#22d3ee', 'rgba(34,211,238,.12)');
  line((p) => p.txBps, '#a78bfa', 'rgba(167,139,250,.10)');
}

// ───────────────────────── Carte ───────────────────────────────────

function nodeHtml(d: Device, x: number, y: number, hub: boolean): string {
  const m = kindMeta(d.kind);
  const cls = ['node', d.online ? 'on' : 'off', d.blocked ? 'blocked' : '', hub ? 'hub' : '', selected === d.id ? 'sel' : ''].join(' ');
  return `<button class="${cls}" data-device="${esc(d.id)}" style="left:${x}%;top:${y}%;--c:${m.color}" title="${esc(deviceName(d))}">
    <span class="ico">${icon(d.kind as IconName, hub ? 32 : 26)}</span><span class="dot"></span>
    <span class="lbl">${esc(deviceName(d))}</span><span class="sub">${esc(d.ip ?? '')}</span></button>`;
}

function renderMap(): void {
  if (!snap) return;
  const devs = snap.devices;
  const hubDev = devs.find((d) => d.isGateway) ?? null;
  const others = devs.filter((d) => d !== hubDev);
  const detail = devs.find((d) => d.id === selected);
  const sig = JSON.stringify([others.map((d) => [d.id, d.online, d.blocked, d.hostname, d.kind]), hubDev?.id, selected]);
  const dsig = JSON.stringify(detail ?? null);
  const key = `map|${sig}|${dsig}`;
  // On ne reconstruit la carte que si quelque chose a réellement changé :
  // sinon les clics seraient perdus à chaque seconde.
  if (mapKey === key) return;
  mapKey = key;

  const online = others.filter((d) => d.online);
  const offline = others.filter((d) => !d.online);
  const place = (list: Device[], r: number, off: number) => list.map((d, i) => {
    const a = (i / Math.max(1, list.length)) * Math.PI * 2 - Math.PI / 2 + off;
    return { d, x: 50 + Math.cos(a) * r * 0.9, y: 50 + Math.sin(a) * r };
  });
  const ring1 = place(online, 34, 0);
  const ring2 = place(offline, 44, 0.4);
  const all = [...ring1, ...ring2];
  const links = all.map(({ d, x, y }) =>
    `<line x1="50%" y1="50%" x2="${x}%" y2="${y}%" class="${!d.online ? 'off' : d.blocked ? 'blk' : ''}"/>`).join('');
  const hubNode = hubDev
    ? nodeHtml(hubDev, 50, 50, true)
    : `<div class="node hub on" style="left:50%;top:50%;--c:#38bdf8"><span class="ico">${icon('router', 32)}</span><span class="lbl">Réseau</span></div>`;

  $('view').innerHTML = `<div class="map-wrap">
    <div class="map-col"><div class="map" id="map"><svg class="links">${links}</svg>${hubNode}${all.map(({ d, x, y }) => nodeHtml(d, x, y, false)).join('')}</div>
      <div class="legend"><span><i style="background:var(--ok)"></i>En ligne</span><span><i style="background:#64748b"></i>Hors ligne</span><span><i style="background:var(--bad)"></i>Bloqué</span></div></div>
    <aside class="panel detail" id="detail"></aside></div>`;
  $('detail').innerHTML = detailHtml(detail);
}

function detailHtml(d?: Device): string {
  if (!d) {
    return `<h2>Détails</h2><div class="empty">Cliquez sur un appareil de la carte pour voir ses informations et le bloquer si besoin.</div>
      ${snap ? `<dl class="kv"><dt>Cette machine</dt><dd>${esc(snap.hostname)}</dd><dt>Adresse</dt><dd>${esc(snap.localIp ?? '—')}</dd><dt>Passerelle</dt><dd>${esc(snap.gateway ?? '—')}</dd><dt>Dernier balayage</dt><dd>${snap.lastScanMs ?? '—'} ms · ${ago(snap.lastScanAt)}</dd></dl>` : ''}`;
  }
  const m = kindMeta(d.kind);
  const ports = (d.ports ?? []).map((p) => `<span class="chip">${p.port}${p.service ? ' · ' + esc(p.service) : ''}</span>`).join('') || '<span class="muted">aucun port détecté</span>';
  const canBlock = !!d.ip && !d.isSelf && !d.isGateway;
  return `<div class="head" style="--c:${m.color}"><span class="ico">${icon(d.kind as IconName, 26)}</span>
      <div><h3>${esc(deviceName(d))}</h3><span class="muted">${m.label}${d.isGateway ? ' · passerelle' : ''}${d.isSelf ? ' · cette machine' : ''}</span></div></div>
    <dl class="kv"><dt>Adresse IP</dt><dd class="mono">${esc(d.ip ?? '—')}</dd><dt>Adresse MAC</dt><dd class="mono">${esc(d.mac ?? '—')}</dd>
      <dt>Constructeur</dt><dd>${esc(d.vendor ?? 'inconnu')}</dd>${d.model ? `<dt>Modèle</dt><dd>${esc(d.model)}</dd>` : ''}
      <dt>Statut</dt><dd>${d.online ? '<span class="tag ok">en ligne</span>' : '<span class="tag">hors ligne</span>'} ${d.blocked ? '<span class="tag bad">bloqué</span>' : ''}</dd>
      <dt>Latence</dt><dd>${d.rttMs != null ? d.rttMs + ' ms' : '—'}</dd><dt>Vu pour la 1ʳᵉ fois</dt><dd>${ago(d.firstSeen)}</dd><dt>Dernière vue</dt><dd>${ago(d.lastSeen)}</dd></dl>
    <h2>Ports ouverts</h2><div class="chips">${ports}</div>
    ${canBlock ? `<p><button class="btn ${d.blocked ? '' : 'danger'}" data-block="${esc(d.id)}" data-to="${d.blocked ? 0 : 1}">${icon(d.blocked ? 'check' : 'ban', 16)}${d.blocked ? 'Débloquer cet appareil' : 'Bloquer cet appareil'}</button></p>
    <p class="muted tiny">La règle est enregistrée ici ; elle ne touche le pare-feu du système qu'après « Appliquer » dans l'onglet Pare-feu.</p>` : ''}`;
}

// ───────────────────────── Appareils ───────────────────────────────

function renderDevices(): void {
  if (!snap) return;
  const q = filterDevices.toLowerCase();
  const list = snap.devices.filter((d) => !q || [d.hostname, d.ip, d.mac, d.vendor, d.kind].join(' ').toLowerCase().includes(q));
  const key = `devices|${q}|${JSON.stringify(list.map((d) => [d.id, d.online, d.blocked, d.rttMs, d.lastSeen > 0]))}`;
  if (renderKey === key) return;
  const focus = document.activeElement?.id === 'q-dev';
  renderKey = key;
  const rows = list.map((d) => {
    const m = kindMeta(d.kind);
    return `<tr><td><span class="cell-ico" style="--c:${m.color}"><span class="ico">${icon(d.kind as IconName, 17)}</span><span><b>${esc(deviceName(d))}</b><br><span class="muted tiny">${m.label}</span></span></span></td>
      <td class="mono">${esc(d.ip ?? '—')}</td><td class="mono">${esc(d.mac ?? '—')}</td><td>${esc(d.vendor ?? '—')}</td>
      <td>${(d.ports ?? []).slice(0, 4).map((p) => `<span class="chip">${p.port}</span>`).join(' ')}</td>
      <td>${d.rttMs != null ? d.rttMs + ' ms' : '—'}</td>
      <td>${d.blocked ? '<span class="tag bad">bloqué</span>' : d.online ? '<span class="tag ok">en ligne</span>' : `<span class="tag">${ago(d.lastSeen)}</span>`}</td></tr>`;
  }).join('');
  $('view').innerHTML = `<div class="toolbar"><input id="q-dev" type="search" placeholder="Rechercher (nom, IP, MAC, marque…)" value="${esc(filterDevices)}">
      <button class="btn" data-export>${icon('download', 16)}Exporter en JSON</button></div>
    <div class="table-wrap"><table><thead><tr><th>Appareil</th><th>IP</th><th>MAC</th><th>Constructeur</th><th>Ports</th><th>Latence</th><th>Statut</th></tr></thead><tbody>${rows || '<tr><td colspan="7" class="empty">Aucun appareil.</td></tr>'}</tbody></table></div>`;
  if (focus) { const i = $('q-dev') as HTMLInputElement; i.focus(); i.setSelectionRange(i.value.length, i.value.length); }
}

// ───────────────────────── Connexions ──────────────────────────────

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
  const tab = (id: string, label: string) => `<button class="btn ${scopeFilter === id ? 'primary' : ''}" data-scope="${id}">${label}</button>`;
  const rows = list.map((c) => `<tr><td><b>${esc(c.process ?? '—')}</b><br><span class="muted tiny">PID ${c.pid ?? '—'}</span></td>
    <td><span class="tag">${c.proto.toUpperCase()}</span></td><td class="mono">${esc(c.localAddr)}:${c.localPort}</td>
    <td class="mono">${c.remotePort ? esc(c.remoteAddr) + ':' + c.remotePort : esc(c.remoteAddr)}</td>
    <td><span class="tag ${c.scope === 'internet' ? 'net' : ''}">${{ internet: 'Internet', lan: 'Réseau local', local: 'Local', multicast: 'Multicast' }[c.scope]}</span></td>
    <td>${c.state === 'listen' ? '<span class="tag warn">en écoute</span>' : esc(c.state)}</td></tr>`).join('');
  $('view').innerHTML = `<div class="toolbar">${tab('all', 'Actives')}${tab('internet', 'Internet')}${tab('lan', 'Réseau local')}${tab('listen', 'En écoute')}
    <input id="q-con" type="search" placeholder="Filtrer (programme, adresse, port…)" value="${esc(filterConns)}"></div>
    <div class="table-wrap"><table><thead><tr><th>Programme</th><th>Proto</th><th>Local</th><th>Distant</th><th>Zone</th><th>État</th></tr></thead><tbody>${rows || '<tr><td colspan="6" class="empty">Aucune connexion.</td></tr>'}</tbody></table></div>`;
  if (focus) { const i = $('q-con') as HTMLInputElement; i.focus(); i.setSelectionRange(i.value.length, i.value.length); }
}

// ───────────────────────── Pare-feu ────────────────────────────────

let fwBuilt = false;

function renderFirewall(): void {
  if (!snap) return;
  if (!fwBuilt || !document.getElementById('fw-list')) {
    fwBuilt = true; renderKey = '';
    $('view').innerHTML = `<div class="panel" id="fw-status">
        <div class="row" style="align-items:center;flex-wrap:wrap">
          <span id="fw-icon"></span>
          <div class="grow"><b id="fw-title"></b> — moteur : <span class="mono" id="fw-engine"></span><br>
            <span class="muted" id="fw-msg"></span><br><span class="muted tiny" id="fw-when"></span></div>
          <button class="btn" data-preview></button>
          <button class="btn primary" data-apply></button>
          <button class="btn danger" data-flush></button>
        </div></div>
      <div class="panel" style="margin-top:14px"><h2>Nouvelle règle</h2>
        <form class="form" id="rule-form">
          <label class="wide">Nom<input name="name" required placeholder="ex. Bloquer le partage Windows"></label>
          <label>Action<select name="action"><option value="block">Bloquer</option><option value="allow">Autoriser</option></select></label>
          <label>Sens<select name="direction"><option value="in">Entrant</option><option value="out">Sortant</option><option value="both">Les deux</option></select></label>
          <label>Protocole<select name="protocol"><option value="tcp">TCP</option><option value="udp">UDP</option><option value="any">Tous</option></select></label>
          <label>Adresse ou réseau<input name="remote" placeholder="192.168.1.42 ou 10.0.0.0/8"></label>
          <label>Ports locaux<input name="localPorts" placeholder="445,139 ou 8000-8100"></label>
          <label>Programme (Windows)<input name="app" placeholder="C:\\Chemin\\app.exe"></label>
          <div><button class="btn primary" type="submit"></button></div>
        </form></div>
      <div class="panel" style="margin-top:14px"><h2>Règles (<span id="fw-count">0</span>)</h2><div id="fw-list"></div></div>`;
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
  $('fw-icon').innerHTML = `<span style="color:${fw.readOnly ? 'var(--warn)' : 'var(--ok)'}">${icon(fw.readOnly ? 'shieldAlert' : 'shieldOk', 34)}</span>`;
  setText('fw-title', fw.readOnly ? 'Lecture seule' : 'Prêt à appliquer');
  setText('fw-engine', fw.backend);
  setText('fw-msg', fw.message);
  setText('fw-when', fw.appliedAt ? `Dernière application ${ago(fw.appliedAt)}` : '');

  const key = `fwlist|${JSON.stringify(rules)}`;
  if ($('fw-list').dataset.key === key) return;
  $('fw-list').dataset.key = key;
  $('fw-count').textContent = String(rules.length);
  $('fw-list').innerHTML = rules.length ? `<div class="table-wrap"><table><thead><tr><th>Actif</th><th>Règle</th><th>Action</th><th>Sens</th><th>Cible</th><th></th></tr></thead><tbody>${rules.map((r) => `<tr>
    <td><input type="checkbox" data-toggle="${esc(r.id)}" ${r.enabled ? 'checked' : ''} style="width:auto" aria-label="Activer"></td>
    <td><b>${esc(r.name)}</b>${r.auto ? ' <span class="tag net">auto</span>' : ''}<br><span class="muted tiny">${esc(r.note ?? '')}</span></td>
    <td><span class="tag ${r.action === 'block' ? 'bad' : 'ok'}">${r.action === 'block' ? 'Bloquer' : 'Autoriser'}</span></td>
    <td>${{ in: 'entrant', out: 'sortant', both: '2 sens' }[r.direction]} · ${r.protocol.toUpperCase()}</td>
    <td class="mono">${esc([r.remote, r.localPorts && 'port ' + r.localPorts, r.app].filter(Boolean).join(' · ') || 'tout')}</td>
    <td><button class="btn ghost" data-del="${esc(r.id)}" title="Supprimer">${icon('trash', 16)}</button></td></tr>`).join('')}</tbody></table></div>`
    : '<div class="empty">Aucune règle. Ajoutez-en une ci-dessus, ou bloquez un appareil depuis la carte.</div>';
}

// ───────────────────────── Journal ─────────────────────────────────

function renderLog(): void {
  if (!snap) return;
  const key = `log|${snap.alerts.map((a) => a.id).join(',')}`;
  if (renderKey === key) return;
  renderKey = key;
  const ic = { info: 'info', warn: 'warn', critical: 'shieldAlert' } as const;
  $('view').innerHTML = `<div class="toolbar"><button class="btn" data-clear>${icon('trash', 16)}Vider le journal</button></div>
    <div class="table-wrap">${snap.alerts.length ? snap.alerts.map((a) => `<div class="alert ${a.level}"><span class="lv">${icon(ic[a.level] as IconName, 18)}</span>
      <div><b>${esc(a.title)}</b><br><span class="muted">${esc(a.detail)}</span></div><time>${clock(a.ts)}</time></div>`).join('') : '<div class="empty">Rien à signaler.</div>'}</div>`;
}

// ───────────────────────── Orchestration ───────────────────────────

function render(): void {
  renderNav(); renderTop();
  ({ map: renderMap, devices: renderDevices, connections: renderConnections, firewall: renderFirewall, log: renderLog })[view]();
}

function setView(v: ViewId): void {
  view = v; renderKey = ''; mapKey = ''; fwBuilt = false; render();
}

let toastTimer = 0;
function toast(msg: string, err = false): void {
  const t = $('toast');
  t.textContent = msg; t.className = `toast ${err ? 'err' : ''}`; t.hidden = false;
  clearTimeout(toastTimer); toastTimer = window.setTimeout(() => { t.hidden = true; }, 5000);
}

async function refreshRules(list?: FirewallRule[]): Promise<void> {
  rules = list ?? await api.rules();
  render();
}

async function showPreview(): Promise<void> {
  const p = await api.preview();
  const d = $('dialog') as HTMLDialogElement;
  d.innerHTML = `<h2 style="margin-top:0">Aperçu — ${esc(p.backend)}</h2><p class="muted">${p.ruleCount} règle(s) active(s). Voici exactement ce qui sera exécuté :</p>
    ${p.warnings.map((w) => `<p class="tag warn" style="display:block;border-radius:8px">${esc(w)}</p>`).join('')}
    <pre class="cmds">${esc(p.commands.join('\n') || '(rien à exécuter)')}</pre>
    <p style="text-align:right"><button class="btn" data-close>Fermer</button></p>`;
  d.showModal();
}

async function runAction(kind: 'apply' | 'flush'): Promise<void> {
  const label = kind === 'apply' ? 'Appliquer les règles au pare-feu du système ?' : 'Retirer toutes les règles SECTOOL du pare-feu du système ?';
  if (!window.confirm(label)) return;
  toast('En cours… une autorisation administrateur peut être demandée.');
  try {
    const r = kind === 'apply' ? await api.apply() : await api.flush();
    toast(r.message, !r.ok);
    await refreshRules(r.rules);
  } catch (e) { toast(String(e), true); }
}

document.addEventListener('click', async (ev) => {
  const el = (ev.target as HTMLElement).closest<HTMLElement>('[data-view],[data-device],[data-block],[data-scope],[data-preview],[data-apply],[data-flush],[data-del],[data-clear],[data-export],[data-close]');
  if (!el) return;
  const ds = el.dataset;
  try {
    if (ds.view) setView(ds.view as ViewId);
    else if (ds.device) { selected = ds.device; mapKey = ''; render(); }
    else if (ds.block) {
      const d = snap?.devices.find((x) => x.id === ds.block);
      if (d?.ip) { await refreshRules(await api.setDeviceBlocked(d.ip, deviceName(d), ds.to === '1')); toast(ds.to === '1' ? 'Règle de blocage créée (à appliquer dans Pare-feu).' : 'Règle retirée.'); }
    }
    else if (ds.scope) { scopeFilter = ds.scope as typeof scopeFilter; renderKey = ''; render(); }
    else if (el.hasAttribute('data-preview')) await showPreview();
    else if (el.hasAttribute('data-apply')) await runAction('apply');
    else if (el.hasAttribute('data-flush')) await runAction('flush');
    else if (ds.del) await refreshRules(await api.deleteRule(ds.del));
    else if (el.hasAttribute('data-clear')) { await api.clearAlerts(); if (snap) snap.alerts = []; renderKey = ''; render(); }
    else if (el.hasAttribute('data-close')) ($('dialog') as HTMLDialogElement).close();
    else if (el.hasAttribute('data-export')) {
      const blob = new Blob([await api.exportReport()], { type: 'application/json' });
      const a = document.createElement('a'); a.href = URL.createObjectURL(blob); a.download = 'sectool-rapport.json'; a.click(); URL.revokeObjectURL(a.href);
    }
  } catch (e) { toast(String(e), true); }
});

document.addEventListener('input', (ev) => {
  const t = ev.target as HTMLInputElement;
  if (t.id === 'q-dev') { filterDevices = t.value; renderKey = ''; renderDevices(); }
  if (t.id === 'q-con') { filterConns = t.value; renderKey = ''; renderConnections(); }
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
      name: v.name, action: v.action as 'block' | 'allow', direction: v.direction as 'in' | 'out' | 'both',
      protocol: v.protocol as 'tcp' | 'udp' | 'any', remote: v.remote, localPorts: v.localPorts, app: v.app, enabled: true,
    }));
    f.reset(); toast('Règle ajoutée.');
  } catch (e) { toast(String(e), true); }
});

$('btn-scan').addEventListener('click', () => { void api.scanNow(); toast('Balayage lancé.'); });
window.addEventListener('resize', drawChart);

if (!isTauri) $('demo-banner').hidden = false;

void (async () => {
  rules = await api.rules();
  await onSnapshot((s) => { snap = s; render(); });
})();
