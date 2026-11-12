// Rewrites the table between <!-- stats:debut --> and <!-- stats:fin --> in README.md from bench/results.json.
// null = "not measured"; the ratio is shown only when both sides are measured. Usage: node bench/table.mjs
import { readFileSync, writeFileSync } from "node:fs";

const dir = new URL("../", import.meta.url);
const r = JSON.parse(readFileSync(new URL("bench/results.json", dir), "utf8"));
const UNIT = { ms: "ms", Mo: "MB", Ko: "kB", nb: "" };
const cell = (v, u) => (v == null ? "not measured" : `${v}${UNIT[u] ? " " + UNIT[u] : ""}`);
const ratio = (l) => {
  if (l.avant == null || l.apres == null || !l.avant || !l.apres) return "";
  const better = l.moins_est_mieux ? l.avant / l.apres : l.apres / l.avant;
  return better >= 1 ? `${+better.toFixed(1)}x better` : `${+(1 / better).toFixed(1)}x worse`;
};
const rows = [
  ...(r.exemple ? ["> **EXAMPLE VALUES, NOT MEASUREMENTS.** Replace `bench/results.json` with real runs, then `node bench/table.mjs && node bench/chart.mjs`.", ""] : []),
  `| | ${r.avant_nom} | \`${r.apres_nom}\` | Ratio | Note |`,
  "|---|---|---|---|---|",
  ...r.lignes.map((l) => `| ${l.metrique} | ${cell(l.avant, l.unite)} | ${cell(l.apres, l.unite)} | ${ratio(l)} | ${l.note ?? ""} |`),
  `| Report (${r.trials ? `${r.trials} trials, ` : ""}${r.env}) | [bench/results.json](bench/results.json) | [bench/chart.svg](bench/chart.svg) | | |`,
];
const p = new URL("README.md", dir);
const txt = readFileSync(p, "utf8");
const B = "<!-- stats:debut -->", E = "<!-- stats:fin -->";
const b = txt.indexOf(B), e = txt.indexOf(E);
if (b < 0 || e < b) throw new Error(`README.md: markers ${B} / ${E} not found`);
writeFileSync(p, `${txt.slice(0, b + B.length)}\n${rows.join("\n")}\n${txt.slice(e)}`);
