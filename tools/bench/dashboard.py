"""
A static HTML dashboard of the benchmark history: each benchmark's trend per language, the gap between
languages, and the reference compilers.

    uv run --project tools python tools/bench/dashboard.py [RESULTS.jsonl] [-o dashboard.html]

RESULTS defaults to results.jsonl on the bench-history branch. The page is one file with its data inline:
open it from disk or serve it anywhere. A gap that widens is a regression, so the first table ranks the
benchmarks by it.
"""

from __future__ import annotations

import sys
import json
import argparse
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]

PAGE = r"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>llrm benchmarks</title>
<style>
:root {
  color-scheme: light;
  --surface: #fcfcfb; --card: #ffffff; --ink: #0b0b0b; --ink-2: #52514e; --ink-3: #6f6e69; --line: #e3e2dd; --grid: #ecebe7;
  --bas: #2a78d6; --c: #eb6834; --nib: #1baf7a;
}
@media (prefers-color-scheme: dark) {
  :root:where(:not([data-theme="light"])) {
    color-scheme: dark;
    --surface: #1a1a19; --card: #222221; --ink: #ffffff; --ink-2: #c3c2b7; --ink-3: #a09f95; --line: #383835; --grid: #2d2d2b;
    --bas: #3987e5; --c: #d95926; --nib: #199e70;
  }
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --surface: #1a1a19; --card: #222221; --ink: #ffffff; --ink-2: #c3c2b7; --ink-3: #a09f95; --line: #383835; --grid: #2d2d2b;
  --bas: #3987e5; --c: #d95926; --nib: #199e70;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--surface); color: var(--ink); font: 14px/1.45 system-ui, sans-serif; }
main { max-width: 1180px; margin: 0 auto; padding: 24px 16px 64px; }
h1 { font-size: 22px; margin: 0 0 4px; }
h2 { font-size: 16px; margin: 32px 0 8px; }
p.sub, .note { color: var(--ink-2); margin: 0 0 12px; }
header { display: flex; justify-content: space-between; align-items: flex-start; gap: 16px; flex-wrap: wrap; }
.controls { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
button, select { font: inherit; color: var(--ink); background: var(--card); border: 1px solid var(--line); border-radius: 6px; padding: 4px 10px; cursor: pointer; }
button[aria-pressed="true"] { border-color: var(--ink-2); font-weight: 600; }
.legend { display: flex; gap: 14px; flex-wrap: wrap; margin: 8px 0 4px; color: var(--ink-2); }
.legend i { display: inline-block; width: 18px; height: 0; border-top: 2px solid; vertical-align: middle; margin-right: 6px; }
.legend i.dash { border-top-style: dashed; }
.grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(340px, 1fr)); gap: 12px; }
.card { background: var(--card); border: 1px solid var(--line); border-radius: 10px; padding: 10px 12px 6px; }
.card h3 { font-size: 13px; margin: 0 0 2px; display: flex; justify-content: space-between; }
.cap { color: var(--ink-3); font-size: 11px; margin-top: 6px; }
.card h3 span { color: var(--ink-3); font-weight: 400; }
svg { display: block; width: 100%; height: auto; overflow: visible; }
svg text { fill: var(--ink-3); font-size: 10px; font-family: inherit; }
.axis line { stroke: var(--grid); }
table { border-collapse: collapse; width: 100%; font-variant-numeric: tabular-nums; }
th, td { text-align: right; padding: 4px 8px; border-bottom: 1px solid var(--line); }
th:first-child, td:first-child { text-align: left; }
th { color: var(--ink-2); font-weight: 600; }
.wider { font-weight: 600; }
#tip { position: fixed; pointer-events: none; background: var(--card); color: var(--ink); border: 1px solid var(--line); border-radius: 6px;
  padding: 6px 8px; font-size: 12px; box-shadow: 0 2px 8px rgba(0,0,0,.18); display: none; max-width: 280px; z-index: 5; }
#tip b { display: block; margin-bottom: 2px; }
details { margin-top: 24px; }
</style>
</head>
<body>
<main>
<header>
  <div><h1>llrm benchmarks</h1><p class="sub" id="summary"></p></div>
  <div class="controls">
    <span>Level</span><button data-opt="O2" aria-pressed="true">-O2</button><button data-opt="Os" aria-pressed="false">-Os</button>
    <button id="theme" aria-label="Toggle colour scheme">Theme</button>
  </div>
</header>
<div class="legend" id="legend"></div>

<h2>Gap between languages</h2>
<p class="note">Executed instructions of each language's kernel, divided by C's, at the latest commit. A gap that widens over the history is a regression. Dashed rows: llrm against the reference compiler for that language.</p>
<table id="gaps"></table>

<h2>Trend per benchmark</h2>
<p class="note">Left: executed instructions per commit. Right: the ratio to C (1 = parity).</p>
<div class="grid" id="cards"></div>

<details><summary>Data table: the latest commit</summary><table id="latest"></table></details>
<div id="tip" role="tooltip"></div>
</main>
<script>
const ROWS = __DATA__;
const LANGS = [["bas","BASIC","--bas",false],["c","C","--c",false],["nib","Nib","--nib",false],["bc","BC 4.5","--bas",true],["ow","Open Watcom","--c",true]];
const css = n => getComputedStyle(document.documentElement).getPropertyValue(n).trim();
let opt = "O2";
const commits = [], seen = new Map();
for (const r of ROWS) if (!seen.has(r.commit)) { seen.set(r.commit, commits.length); commits.push({sha: r.commit, date: r.date, subject: r.subject}); }
const names = [...new Set(ROWS.map(r => r.benchmark))].sort();
const at = (b, l, o, ci) => { const r = ROWS.find(x => x.benchmark === b && x.language === l && x.opt === o && seen.get(x.commit) === ci); return r && r.instructions != null ? r.instructions : null; };
const series = (b, l, o) => commits.map((_, i) => at(b, l, o, i));
const refOpt = l => l === "bc" ? "O2" : opt;
const fmt = n => n == null ? "–" : n >= 1e6 ? (n / 1e6).toFixed(2) + "M" : n >= 1e4 ? (n / 1e3).toFixed(1) + "k" : String(n);
const ratio = (a, b) => a == null || b == null || b === 0 ? null : a / b;
const last = a => { for (let i = a.length - 1; i >= 0; i--) if (a[i] != null) return a[i]; return null; };
const first = a => a.find(v => v != null) ?? null;

function legend() {
  document.getElementById("legend").innerHTML = LANGS.map(([k, label, v, dash]) =>
    `<span><i class="${dash ? "dash" : ""}" style="border-color:var(${v})"></i>${label}${dash ? " (reference)" : ""}</span>`).join("");
}

function chart(title, lines, ref, unit) {
  const W = 330, H = 150, L = 38, R = 30, T = 8, B = 18, n = commits.length;
  const all = lines.flatMap(l => l.v).filter(v => v != null);
  if (!all.length) return `<div class="note">no data</div>`;
  const lo = ref != null ? Math.min(ref, ...all) : 0, hi = Math.max(ref ?? 0, ...all) * 1.08 || 1, base = ref != null ? Math.min(lo, ref) * 0.95 : 0;
  const x = i => n === 1 ? (L + W - R) / 2 : L + (W - L - R) * i / (n - 1);
  const y = v => T + (H - T - B) * (1 - (v - base) / (hi - base));
  let g = "";
  for (let k = 0; k <= 3; k++) { const v = base + (hi - base) * k / 3; g += `<line x1="${L}" x2="${W - R}" y1="${y(v)}" y2="${y(v)}"/><text x="${L - 4}" y="${y(v) + 3}" text-anchor="end">${unit ? v.toFixed(2) : fmt(Math.round(v))}</text>`; }
  if (ref != null) g += `<line x1="${L}" x2="${W - R}" y1="${y(ref)}" y2="${y(ref)}" style="stroke:var(--ink-3);stroke-dasharray:2 3"/>`;
  let body = "";
  for (const l of lines) {
    const pts = l.v.map((v, i) => v == null ? null : [x(i), y(v)]).filter(Boolean);
    if (pts.length > 1) body += `<path d="M${pts.map(p => p.join(",")).join("L")}" fill="none" stroke="var(${l.color})" stroke-width="2" ${l.dash ? 'stroke-dasharray="5 4"' : ""}/>`;
    for (const p of pts) body += `<circle cx="${p[0]}" cy="${p[1]}" r="${pts.length > 1 ? 2.5 : 4}" fill="var(${l.color})" stroke="var(--card)" stroke-width="2"/>`;
    if (pts.length) body += `<text x="${pts.at(-1)[0] + 6}" y="${pts.at(-1)[1] + 3}" style="fill:var(--ink-2)">${l.label}</text>`;
  }
  const hover = `<rect class="hit" x="${L}" y="${T}" width="${W - L - R}" height="${H - T - B}" fill="transparent"/>`;
  return `<svg viewBox="0 0 ${W} ${H}" role="img" aria-label="${title}" data-lines='${JSON.stringify(lines.map(l => ({label: l.label, v: l.v})))}' data-unit="${unit ? 1 : 0}"><g class="axis">${g}</g>${body}${hover}</svg>`;
}

function cards() {
  const out = [];
  for (const b of names) {
    const s = Object.fromEntries(LANGS.map(([k]) => [k, series(b, k, refOpt(k))]));
    const lines = LANGS.filter(([k]) => s[k].some(v => v != null)).map(([k, label, v, dash]) => ({label: k === "bc" ? "BC" : k === "ow" ? "OW" : label, v: s[k], color: v, dash}));
    const rel = (a, d) => commits.map((_, i) => ratio(a[i], d[i]));
    const gaps = [{label: "BASIC/C", v: rel(s.bas, s.c), color: "--bas"}, {label: "Nib/C", v: rel(s.nib, s.c), color: "--nib"},
                  {label: "C/OW", v: rel(s.c, s.ow), color: "--c", dash: true}, {label: "BASIC/BC", v: rel(s.bas, s.bc), color: "--bas", dash: true}].filter(l => l.v.some(v => v != null));
    out.push(`<div class="card"><h3>${b}<span>${commits.length} commit${commits.length > 1 ? "s" : ""}</span></h3><div class="cap">executed instructions</div>${chart(b + " instructions", lines, null, false)}<div class="cap">ratio to C</div>${chart(b + " ratio to C", gaps, 1, true)}</div>`);
  }
  document.getElementById("cards").innerHTML = out.join("");
}

function gapTable() {
  const rows = names.map(b => {
    const c = series(b, "c", opt), cell = (l) => { const a = series(b, l, refOpt(l)); return [ratio(last(a), last(c)), ratio(first(a), first(c))]; };
    const bas = cell("bas"), nib = cell("nib");
    const ow = (() => { const a = series(b, "ow", opt); return ratio(last(c), last(a)); })(), bc = (() => { const a = series(b, "bc", "O2"); return ratio(last(series(b, "bas", "O2")), last(a)); })();
    return {b, bas, nib, ow, bc, worst: Math.max(bas[0] ?? 0, nib[0] ?? 0)};
  }).sort((p, q) => q.worst - p.worst);
  const f = v => v == null ? "–" : v.toFixed(2) + "×";
  const trend = ([now, then]) => now == null || then == null || commits.length < 2 ? "" : now > then * 1.005 ? " ▲ wider" : now < then * 0.995 ? " ▼ narrower" : " = same";
  const cls = ([now, then]) => now != null && then != null && now > then * 1.005 ? "wider" : "";
  document.getElementById("gaps").innerHTML = `<tr><th>Benchmark</th><th>BASIC ÷ C</th><th>Nib ÷ C</th><th>C ÷ Open Watcom</th><th>BASIC ÷ BC</th></tr>` +
    rows.map(r => `<tr><td>${r.b}</td><td class="${cls(r.bas)}">${f(r.bas[0])}${trend(r.bas)}</td><td class="${cls(r.nib)}">${f(r.nib[0])}${trend(r.nib)}</td><td>${f(r.ow)}</td><td>${f(r.bc)}</td></tr>`).join("");
}

function latestTable() {
  const head = LANGS.map(([k, l]) => `<th>${l}</th>`).join("");
  const li = commits.length - 1;
  document.getElementById("latest").innerHTML = `<tr><th>Benchmark (${commits[li].sha.slice(0, 8)}, -${opt})</th>${head}</tr>` +
    names.map(b => `<tr><td>${b}</td>${LANGS.map(([k]) => `<td>${fmt(at(b, k, refOpt(k), li))}</td>`).join("")}</tr>`).join("");
}

function hover() {
  const tip = document.getElementById("tip");
  for (const svg of document.querySelectorAll("svg[data-lines]")) {
    const lines = JSON.parse(svg.dataset.lines), unit = svg.dataset.unit === "1", n = commits.length, L = 38, R = 30, W = 330;
    svg.addEventListener("pointermove", e => {
      const box = svg.getBoundingClientRect(), px = (e.clientX - box.left) / box.width * W;
      const i = n === 1 ? 0 : Math.max(0, Math.min(n - 1, Math.round((px - L) / (W - L - R) * (n - 1))));
      const c = commits[i];
      tip.innerHTML = `<b>${c.sha.slice(0, 8)} ${c.subject}</b>` + lines.map(l => `${l.label}: ${l.v[i] == null ? "–" : unit ? l.v[i].toFixed(3) + "×" : l.v[i].toLocaleString()}`).join("<br>");
      tip.style.display = "block"; tip.style.left = Math.min(e.clientX + 12, innerWidth - 300) + "px"; tip.style.top = e.clientY + 12 + "px";
    });
    svg.addEventListener("pointerleave", () => tip.style.display = "none");
  }
}

function draw() { cards(); gapTable(); latestTable(); hover(); }
document.getElementById("summary").textContent = commits.length ? `${names.length} benchmarks, ${commits.length} commit${commits.length > 1 ? "s" : ""}, ${commits[0].date.slice(0, 10)} to ${commits.at(-1).date.slice(0, 10)}` : "no data yet";
for (const b of document.querySelectorAll("[data-opt]")) b.addEventListener("click", () => { opt = b.dataset.opt; document.querySelectorAll("[data-opt]").forEach(o => o.setAttribute("aria-pressed", o === b)); draw(); });
document.getElementById("theme").addEventListener("click", () => { const r = document.documentElement; const dark = r.dataset.theme ? r.dataset.theme === "dark" : matchMedia("(prefers-color-scheme: dark)").matches; r.dataset.theme = dark ? "light" : "dark"; });
legend(); draw();
</script>
</body>
</html>
"""


def load(source: str | None) -> list[dict]:
    text = Path(source).read_text() if source else subprocess.run(["git", "show", "bench-history:results.jsonl"], cwd=ROOT, capture_output=True, text=True, check=True).stdout
    return [json.loads(line) for line in text.splitlines() if line.strip()]


def render(rows: list[dict]) -> str:
    return PAGE.replace("__DATA__", json.dumps(rows, separators=(",", ":")).replace("</", "<\\/"))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("results", nargs="?")
    parser.add_argument("-o", "--output", type=Path, default=ROOT / "target" / "bench" / "dashboard.html")
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(render(load(args.results)))
    print(args.output)
    return 0


if __name__ == "__main__":
    sys.exit(main())
