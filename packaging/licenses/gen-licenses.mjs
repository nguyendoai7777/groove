#!/usr/bin/env node
// Bản Node của `gen-licenses.ps1` cho máy không có `pwsh` (Linux, macOS): cùng các bước, cùng chốt soát (D-2), cùng nội dung ra.
// Sinh `<Out>/third-party.html`: cây phụ thuộc của 6 target phát hành (`about.toml`, cùng danh sách `deny.toml`), gộp theo
// giấy phép, đủ văn bản, cộng thứ cargo không thấy (mảng `extra` dưới đây). `cargo deny check licenses` đỏ ⇒ dừng.
//   cargo install cargo-about --locked --features cli
//   node packaging/licenses/gen-licenses.mjs dist/licenses
// Hai bản phải giữ khớp nhau: sửa `extra`, `required` hay luật soát ở bản này thì sửa cả `gen-licenses.ps1`.
import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const out = process.argv[2];
if (!out || process.argv.length !== 3) fail('cách dùng: node packaging/licenses/gen-licenses.mjs <thư mục ra>');

const dir = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(dir, '../..');

// G-2: `libasound` là ngoại lệ LGPL duy nhất, chỉ link động và không nằm trong bản cài, nên có mặt trên trang mà không kèm tệp.
const extra = [
  {
    name: 'ALSA library (libasound)', version: 'provided by the system', license: 'LGPL-2.1-or-later',
    url: 'https://github.com/alsa-project/alsa-lib', text: 'libasound.txt',
  },
  {
    name: 'Lucide', version: 'icons used by the materiant library', license: 'ISC',
    url: 'https://lucide.dev', text: '../../../materiant/assets/icons/LICENSE-lucide',
  },
  {
    name: 'Mona Sans', version: 'font embedded by the materiant library', license: 'OFL-1.1',
    url: 'https://github.com/github/mona-sans', text: '../../../materiant/assets/fonts/mona-sans/OFL.txt',
  },
  {
    name: 'Be Vietnam Pro', version: 'font embedded by the materiant library', license: 'OFL-1.1',
    url: 'https://github.com/bettergui/BeVietnamPro', text: '../../../materiant/assets/fonts/be-vietnam-pro/OFL.txt',
  },
];
// Nhãn được phép ngoài danh sách D-2: chỉ đúng thành phần G-2 đã chấp nhận.
const lgplAllowed = 'ALSA library (libasound)';
// Thứ phải có: một target rơi khỏi cây thì tên của nó biến mất.
const required = ['gpui-pre', 'gpui-kit', 'cpal', 'alsa-sys', 'souvlaki', 'windows', 'objc2'];

function fail(message) {
  console.error(`gen-licenses: ${message}`);
  process.exit(1);
}

function run(args) {
  const r = spawnSync('cargo', args, { cwd: repo, stdio: 'inherit' });
  if (r.status !== 0) fail(`cargo ${args[0]} failed (exit ${r.status})`);
}

function targets(file) {
  const block = /^targets\s*=\s*\[([\s\S]*?)^\]/m.exec(fs.readFileSync(file, 'utf8'));
  if (!block) fail(`no targets in ${file}`);
  return [...block[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]).sort();
}

// Như `System.Net.WebUtility.HtmlEncode` của bản pwsh: thêm cả U+00A0–U+00FF dạng số, để hai bản ra cùng một tệp.
function escapeHtml(text) {
  return text.replace(/[&<>"' -ÿ]/g, (ch) => {
    switch (ch) {
      case '&': return '&amp;';
      case '<': return '&lt;';
      case '>': return '&gt;';
      case '"': return '&quot;';
      case "'": return '&#39;';
      default: return `&#${ch.charCodeAt(0)};`;
    }
  });
}

function decodeHtml(text) {
  return text
    .replace(/&#(\d+);/g, (_, n) => String.fromCharCode(Number(n)))
    .replace(/&quot;/g, '"').replace(/&lt;/g, '<').replace(/&gt;/g, '>').replace(/&amp;/g, '&');
}

const readText = (file) => fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
const unique = (items) => [...new Set(items)].sort();

// Hai danh sách target lệch nhau ⇒ trang giấy phép không còn khớp thứ `cargo deny` đã soát.
if (targets(path.join(dir, 'about.toml')).join(',') !== targets(path.join(repo, 'deny.toml')).join(',')) {
  fail('targets differ: about.toml vs deny.toml');
}

// `--frozen`: không gọi ClearlyDefined, chỉ đọc nguồn crate đã tải, chạy lại ra cùng nội dung.
run(['fetch', '--locked']);
run(['deny', '--log-level', 'error', 'check', 'licenses']);
const raw = path.join(os.tmpdir(), `groove-licenses-${crypto.randomUUID().replace(/-/g, '')}.html`);
let html;
try {
  run(['about', 'generate', '--frozen', '--workspace', '--config', path.join(dir, 'about.toml'), '--output-file', raw,
    path.join(dir, 'template.hbs')]);
  html = readText(raw);
} finally {
  fs.rmSync(raw, { force: true });
}

const articles = extra.map((c) => [
  `<article data-license="${escapeHtml(c.license)}" data-component="${escapeHtml(c.name)}">`,
  `<h3>${escapeHtml(c.name)} ${escapeHtml(c.version)}: ${escapeHtml(c.license)}</h3>`,
  `<p class="note"><a href="${escapeHtml(c.url)}">${escapeHtml(c.url)}</a></p>`,
  `<pre>${escapeHtml(readText(path.resolve(dir, c.text)))}</pre>`,
  '</article>',
].join('\n'));
const marker = '<!-- extra-components -->';
if (!html.includes(marker)) fail(`template has no ${marker}`);
html = html.replace(marker, () => articles.join('\n'));

// D-2: soát nhãn (không soát cả văn bản: MPL có nhắc tên GPL mà không áp dụng). Riêng crate thì nhãn LGPL nào cũng là lỗi.
const gpl = /\b(a|l)?gpl/i;
const crateLabels = unique([...html.matchAll(/<article data-license="([^"]+)">/g)].map((m) => m[1]));
const banned = crateLabels.filter((label) => gpl.test(label));
if (banned.length) fail(`banned license: ${banned.join(', ')}`);
const bannedComponents = [...html.matchAll(/data-license="([^"]+)" data-component="([^"]+)"/g)]
  .map((m) => ({ license: m[1], name: decodeHtml(m[2]) }))
  .filter((c) => gpl.test(c.license) && c.name !== lgplAllowed);
if (bannedComponents.length) fail(`banned license: ${bannedComponents.map((c) => c.name).join(', ')}`);

const crates = unique([...html.matchAll(/data-crate="([^"]+)"/g)].map((m) => m[1]));
const absent = required.filter((name) => !crates.includes(name));
if (absent.length) fail(`missing from the page: ${absent.join(', ')}`);

fs.mkdirSync(out, { recursive: true });
const file = path.join(path.resolve(out), 'third-party.html');
fs.writeFileSync(file, html);
console.log(`crates=${crates.length} licenses=${crateLabels.join(', ')}`);
console.log(`sha256=${crypto.createHash('sha256').update(html).digest('hex')}`);
console.log(`wrote ${file}`);
