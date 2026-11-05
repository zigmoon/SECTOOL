// Télécharge une seule fois les polices utilisées par l'interface et génère
// `app/src/styles/fonts.css` avec des chemins locaux (/fonts/...).
// Les polices sont sous licence SIL Open Font License 1.1 (Google Fonts).

import { writeFile, mkdir } from 'node:fs/promises';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, '..');
const outDir = resolve(root, 'public/fonts');
const cssOut = resolve(root, 'src/styles/fonts.css');

const UA = 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0 Safari/537.36';
const URL_CSS = 'https://fonts.googleapis.com/css2?family=Space+Grotesk:wght@400;500;600;700&family=JetBrains+Mono:wght@400;500;600&display=swap';

await mkdir(outDir, { recursive: true });
const css = await (await fetch(URL_CSS, { headers: { 'User-Agent': UA } })).text();

const BLOCK = /\/\*\s*([a-z-]+)\s*\*\/\s*@font-face\s*{([^}]+)}/g;
const faces = [];
let m;
while ((m = BLOCK.exec(css))) {
  const subset = m[1];
  if (subset !== 'latin') continue;
  const body = m[2];
  const family = /font-family:\s*'([^']+)'/.exec(body)?.[1];
  const weight = /font-weight:\s*(\d+)/.exec(body)?.[1];
  const url = /url\((https:[^)]+)\)/.exec(body)?.[1];
  if (family && weight && url) faces.push({ family, weight, url });
}

const slug = (f) => f.toLowerCase().replace(/\s+/g, '-');
// Google Fonts sert une police variable : un seul fichier couvre toutes les
// graisses. On ne le télécharge qu'une fois par famille.
const seen = new Map();
const lines = ['/* Polices locales — SIL Open Font License 1.1 (Google Fonts) */'];
for (const face of faces) {
  if (seen.has(face.family)) continue;
  const file = `${slug(face.family)}.woff2`;
  seen.set(face.family, file);
  const res = await fetch(face.url, { headers: { 'User-Agent': UA } });
  await writeFile(resolve(outDir, file), Buffer.from(await res.arrayBuffer()));
  lines.push(`@font-face {
  font-family: '${face.family}';
  font-style: normal;
  font-weight: 300 700;
  font-display: swap;
  src: url('/fonts/${file}') format('woff2');
}`);
}
await writeFile(cssOut, lines.join('\n\n') + '\n');
console.log(`${seen.size} fichiers de police téléchargés, fonts.css écrit.`);
