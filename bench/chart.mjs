// Generates bench/chart.svg from bench/results.json: one panel per metric, before vs after.
// null = "not measured" (no bar). "exemple": true adds a visible EXAMPLE banner.
// Usage: node bench/chart.mjs [results.json] [chart.svg]
import { readFileSync, writeFileSync } from "node:fs";

const dir = new URL("./", import.meta.url);
const r = JSON.parse(readFileSync(process.argv[2] ?? new URL("results.json", dir), "utf8"));
const UNIT = { ms: "ms", Mo: "MB", Ko: "kB", nb: "" };
const fmt = (v, u) => (v == null ? "not measured" : `${v >= 100 ? Math.round(v) : +v.toFixed(1)}${UNIT[u] ? " " + UNIT[u] : ""}`);
const esc = (t) => String(t).replace(/&/g, "&amp;").replace(/</g, "&lt;");
const W = 720, PAD = 24, LAB = 170, BARW = W - PAD * 2 - LAB - 110, ROW = 22, GAP = 4, PH = 2 * ROW + GAP + 40;
const top = r.exemple ? 120 : 96, H = top + r.lignes.length * PH + 24;
let y = top, body = "";
for (const l of r.lignes) {
  const max = Math.max(l.avant ?? 0, l.apres ?? 0) || 1;
  body += `<text x="${PAD}" y="${y + 12}" class="t2">${esc(l.metrique)}${l.moins_est_mieux ? "" : " (higher is better)"}</text>`;
  [[l.avant, "s1", r.avant_nom], [l.apres, "s2", r.apres_nom]].forEach(([v, c, name], i) => {
    const by = y + 22 + i * (ROW + GAP), w = Math.max(3, ((v ?? 0) / max) * BARW);
    body += `<text x="${PAD + LAB - 8}" y="${by + 15}" text-anchor="end" class="t2 sm">${esc(name)}</text>`;
    if (v == null) body += `<text x="${PAD + LAB}" y="${by + 15}" class="mu">not measured</text>`;
    else body += `<path d="M${PAD + LAB},${by}H${PAD + LAB + w - 4}a4,4 0 0 1 4,4V${by + ROW - 4}a4,4 0 0 1 -4,4H${PAD + LAB}Z" class="${c}"/><text x="${PAD + LAB + w + 8}" y="${by + 15}" class="t1">${esc(fmt(v, l.unite))}</text>`;
  });
  y += PH;
  body += `<line x1="${PAD}" x2="${W - PAD}" y1="${y - 14}" y2="${y - 14}" class="grid"/>`;
}
const banner = r.exemple ? `<rect x="${PAD}" y="88" width="${W - PAD * 2}" height="22" rx="3" class="warn"/><text x="${W / 2}" y="103" text-anchor="middle" class="wt">EXAMPLE VALUES, NOT MEASUREMENTS</text>` : "";
const svg = `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}" role="img" aria-label="Benchmark: ${esc(r.avant_nom)} against ${esc(r.apres_nom)}">
<style>
svg{--bg:#fcfcfb;--t1:#0b0b0b;--t2:#52514e;--mu:#898781;--grid:#e1e0d9;--s1:#2a78d6;--s2:#eb6834;--warn:#e5172a}
@media (prefers-color-scheme:dark){svg{--bg:#1a1a19;--t1:#f0efec;--t2:#c3c2b7;--mu:#898781;--grid:#2c2c2a;--s1:#3987e5;--s2:#d95926}}
text{font-family:system-ui,-apple-system,"Segoe UI",sans-serif}
.bg{fill:var(--bg)}.t1{fill:var(--t1);font-size:13px;font-weight:600}.t2{fill:var(--t2);font-size:13px}.sm{font-size:12px}.mu{fill:var(--mu);font-size:11px}
.h{fill:var(--t1);font-size:18px;font-weight:600}.grid{stroke:var(--grid);stroke-width:1}.s1{fill:var(--s1)}.s2{fill:var(--s2)}.warn{fill:var(--warn)}.wt{fill:#fff;font-size:12px;font-weight:700;letter-spacing:2px}
</style>
<rect class="bg" width="${W}" height="${H}"/>
<text x="${PAD}" y="34" class="h">${esc(r.avant_nom)} against ${esc(r.apres_nom)}</text>
<text x="${PAD}" y="54" class="t2">${esc(r.env)}${r.trials ? ` · ${r.trials} trials` : ""}</text>
<rect x="${PAD}" y="66" width="10" height="10" rx="2" class="s1"/><text x="${PAD + 16}" y="75" class="t2 sm">${esc(r.avant_nom)}</text>
<rect x="${PAD + 260}" y="66" width="10" height="10" rx="2" class="s2"/><text x="${PAD + 276}" y="75" class="t2 sm">${esc(r.apres_nom)}</text>
${banner}${body}</svg>
`;
writeFileSync(process.argv[3] ?? new URL("chart.svg", dir), svg);
