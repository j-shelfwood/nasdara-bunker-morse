#!/usr/bin/env bun
// Builds the outline of the Nasdara island for the minimap from the positions of its buildings and spawn zones.
//
//   bun tools/nasdara-outline.ts <worlds_nasdara_ce dir> [--svg preview.svg]
//
// The input is the mission's `mapgrouppos.xml` (every building group) and `env/*_territories.xml` (spawn zones). Where
// buildings and spawn zones exist, there is land. The positions are counted into a coarse grid, blurred, cut at two
// densities (the coast, and built-up areas) and the edge of each shape is traced, smoothed and simplified. Only the
// resulting outlines are written, to `crates/app/src/ui/island.rs`; no positions are kept.
import { readFileSync, readdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { parseArgs } from "node:util";

const MAP = 16384;
const N = 128; // cells per side, 128 m each
const CELL = MAP / N;

const { values, positionals } = parseArgs({
  args: Bun.argv.slice(2),
  options: { svg: { type: "string" } },
  allowPositionals: true,
});
const dir = positionals[0];
if (!dir) {
  console.error("usage: nasdara-outline.ts <worlds_nasdara_ce dir> [--svg file]");
  process.exit(2);
}

type Pt = [number, number];

function positions(): Pt[] {
  const out: Pt[] = [];
  const groups = readFileSync(join(dir, "mapgrouppos.xml"), "utf8");
  for (const m of groups.matchAll(/pos="(-?[\d.]+) -?[\d.]+ (-?[\d.]+)"/g)) out.push([+m[1], +m[2]]);
  for (const f of readdirSync(join(dir, "env")).filter((f) => f.endsWith("_territories.xml"))) {
    const xml = readFileSync(join(dir, "env", f), "utf8");
    for (const m of xml.matchAll(/ x="(-?[\d.]+)" z="(-?[\d.]+)"/g)) out.push([+m[1], +m[2]]);
  }
  return out.filter(([x, z]) => x >= 0 && x < MAP && z >= 0 && z < MAP);
}

/** Counts per cell, row 0 at the north edge (z = MAP). */
function density(pts: Pt[]): Float64Array {
  const g = new Float64Array(N * N);
  for (const [x, z] of pts) {
    const col = Math.min(N - 1, Math.floor(x / CELL));
    const row = Math.min(N - 1, Math.floor((MAP - z) / CELL));
    g[row * N + col] += 1;
  }
  return g;
}

function blur(g: Float64Array, sigma: number): Float64Array {
  const r = Math.ceil(sigma * 3);
  const k = Array.from({ length: 2 * r + 1 }, (_, i) => Math.exp(-((i - r) ** 2) / (2 * sigma * sigma)));
  const sum = k.reduce((a, b) => a + b, 0);
  const pass = (src: Float64Array, horizontal: boolean) => {
    const dst = new Float64Array(N * N);
    for (let y = 0; y < N; y++)
      for (let x = 0; x < N; x++) {
        let acc = 0;
        for (let i = -r; i <= r; i++) {
          const xx = horizontal ? x + i : x;
          const yy = horizontal ? y : y + i;
          if (xx >= 0 && xx < N && yy >= 0 && yy < N) acc += src[yy * N + xx] * k[i + r];
        }
        dst[y * N + x] = acc / sum;
      }
    return dst;
  };
  return pass(pass(g, true), false);
}

/** Grows then shrinks a mask, closing narrow gaps. */
function close(mask: Uint8Array, steps: number): Uint8Array {
  const morph = (m: Uint8Array, grow: boolean) => {
    const out = new Uint8Array(N * N);
    for (let y = 0; y < N; y++)
      for (let x = 0; x < N; x++) {
        let any = false;
        let all = true;
        for (const [dx, dy] of [[0, 0], [1, 0], [-1, 0], [0, 1], [0, -1]]) {
          const xx = x + dx;
          const yy = y + dy;
          const v = xx >= 0 && xx < N && yy >= 0 && yy < N ? m[yy * N + xx] : 0;
          if (v) any = true;
          else all = false;
        }
        out[y * N + x] = (grow ? any : all) ? 1 : 0;
      }
    return out;
  };
  let m = mask;
  for (let i = 0; i < steps; i++) m = morph(m, true);
  for (let i = 0; i < steps; i++) m = morph(m, false);
  return m;
}

/** Cells of a mask in connected pieces (4-neighbour), each as a list of cell indices. */
function components(mask: Uint8Array): number[][] {
  const seen = new Uint8Array(N * N);
  const out: number[][] = [];
  for (let s = 0; s < N * N; s++) {
    if (!mask[s] || seen[s]) continue;
    const stack = [s];
    const cells: number[] = [];
    seen[s] = 1;
    while (stack.length) {
      const c = stack.pop() as number;
      cells.push(c);
      const x = c % N;
      const y = (c - x) / N;
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const xx = x + dx;
        const yy = y + dy;
        if (xx < 0 || xx >= N || yy < 0 || yy >= N) continue;
        const n = yy * N + xx;
        if (mask[n] && !seen[n]) {
          seen[n] = 1;
          stack.push(n);
        }
      }
    }
    out.push(cells);
  }
  return out;
}

/** Fills background areas that do not touch the grid edge. */
function fillHoles(mask: Uint8Array): Uint8Array {
  const inverse = mask.map((v) => (v ? 0 : 1)) as Uint8Array;
  const out = new Uint8Array(mask);
  for (const piece of components(inverse)) {
    const touchesEdge = piece.some((c) => c % N === 0 || c % N === N - 1 || c < N || c >= N * (N - 1));
    if (!touchesEdge) for (const c of piece) out[c] = 1;
  }
  return out;
}

/** Outer boundaries of one connected piece as closed loops of grid corners, counter-clockwise around the cells. */
function trace(cells: number[]): Pt[][] {
  const inside = new Set(cells);
  const has = (x: number, y: number) => x >= 0 && x < N && y >= 0 && y < N && inside.has(y * N + x);
  const edges = new Map<string, Pt[]>();
  const add = (a: Pt, b: Pt) => {
    const key = `${a[0]},${a[1]}`;
    edges.set(key, [...(edges.get(key) ?? []), b]);
  };
  for (const c of cells) {
    const x = c % N;
    const y = (c - x) / N;
    if (!has(x, y - 1)) add([x, y], [x + 1, y]);
    if (!has(x + 1, y)) add([x + 1, y], [x + 1, y + 1]);
    if (!has(x, y + 1)) add([x + 1, y + 1], [x, y + 1]);
    if (!has(x - 1, y)) add([x, y + 1], [x, y]);
  }
  const loops: Pt[][] = [];
  while (edges.size) {
    const [startKey] = edges.keys();
    const start = startKey.split(",").map(Number) as Pt;
    const loop: Pt[] = [start];
    let cur = start;
    let heading: Pt = [0, 0];
    for (;;) {
      const key = `${cur[0]},${cur[1]}`;
      const out = edges.get(key);
      if (!out) break;
      // at a corner touched twice, turn the same way each time so the loops stay separate
      let pick = 0;
      if (out.length > 1) {
        const turn = (p: Pt) => heading[0] * (p[1] - cur[1]) - heading[1] * (p[0] - cur[0]);
        pick = out.map(turn).indexOf(Math.min(...out.map(turn)));
      }
      const next = out[pick];
      out.splice(pick, 1);
      if (!out.length) edges.delete(key);
      heading = [next[0] - cur[0], next[1] - cur[1]];
      cur = next;
      if (cur[0] === start[0] && cur[1] === start[1]) break;
      loop.push(cur);
    }
    loops.push(loop);
  }
  return loops;
}

/** Pulls each point toward the middle of its neighbours, which wears the grid's steps down to a coastline. */
function relax(loop: Pt[], rounds: number): Pt[] {
  let pts = loop;
  for (let r = 0; r < rounds; r++) {
    pts = pts.map((p, i) => {
      const a = pts[(i + pts.length - 1) % pts.length];
      const b = pts[(i + 1) % pts.length];
      return [0.5 * p[0] + 0.25 * (a[0] + b[0]), 0.5 * p[1] + 0.25 * (a[1] + b[1])] as Pt;
    });
  }
  return pts;
}

function chaikin(loop: Pt[], rounds: number): Pt[] {
  let pts = loop;
  for (let r = 0; r < rounds; r++) {
    const next: Pt[] = [];
    for (let i = 0; i < pts.length; i++) {
      const a = pts[i];
      const b = pts[(i + 1) % pts.length];
      next.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
      next.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
    }
    pts = next;
  }
  return pts;
}

function simplify(pts: Pt[], tolerance: number): Pt[] {
  const keep = new Uint8Array(pts.length);
  keep[0] = 1;
  keep[pts.length - 1] = 1;
  const stack: [number, number][] = [[0, pts.length - 1]];
  while (stack.length) {
    const [a, b] = stack.pop() as [number, number];
    let far = -1;
    let dist = 0;
    for (let i = a + 1; i < b; i++) {
      const [px, py] = pts[i];
      const [ax, ay] = pts[a];
      const [bx, by] = pts[b];
      const len = Math.hypot(bx - ax, by - ay) || 1;
      const d = Math.abs((bx - ax) * (ay - py) - (ax - px) * (by - ay)) / len;
      if (d > dist) {
        dist = d;
        far = i;
      }
    }
    if (far >= 0 && dist > tolerance) {
      keep[far] = 1;
      stack.push([a, far], [far, b]);
    }
  }
  return pts.filter((_, i) => keep[i]);
}

/** A shape's outlines in metres. */
function outlines(field: Float64Array, threshold: number, minCells: number, closing: number): Pt[][] {
  let mask = new Uint8Array(N * N);
  for (let i = 0; i < N * N; i++) mask[i] = field[i] >= threshold ? 1 : 0;
  mask = fillHoles(close(mask, closing));
  const out: Pt[][] = [];
  for (const piece of components(mask)) {
    if (piece.length < minCells) continue;
    for (const loop of trace(piece)) {
      const smooth = simplify(relax(chaikin(loop, 1), 6), 0.12);
      if (smooth.length < 4) continue;
      out.push(smooth.map(([x, y]) => [Math.round(x * CELL), Math.round(MAP - y * CELL)] as Pt));
    }
  }
  return out;
}

const pts = positions();
const grid = density(pts);
const coast = outlines(blur(grid, 1.4), 0.35, 12, 2);
const built = outlines(blur(grid, 1.0), 3.0, 3, 1);
console.error(`${pts.length} positions, ${coast.length} coast loops (${coast.reduce((a, l) => a + l.length, 0)} points), ${built.length} built-up loops (${built.reduce((a, l) => a + l.length, 0)} points)`);

const rustLoops = (loops: Pt[][]) =>
  loops.map((l) => `    &[${l.map(([x, z]) => `(${x}.0, ${z}.0)`).join(", ")}],`).join("\n");
writeFileSync(
  new URL("../crates/app/src/ui/island.rs", import.meta.url),
  `//! Outline of the Nasdara island for the minimap, in game metres (x east, z north). Generated by
//! \`tools/nasdara-outline.ts\` from the positions of the island's buildings and spawn zones; do not edit by hand.

/// Closed loops around the land.
pub const COAST: &[&[(f32, f32)]] = &[
${rustLoops(coast)}
];

/// Closed loops around the densely built-up areas.
pub const BUILT_UP: &[&[(f32, f32)]] = &[
${rustLoops(built)}
];
`,
);

if (values.svg) {
  const path = (l: Pt[]) => "M" + l.map(([x, z]) => `${x / 16},${(MAP - z) / 16}`).join("L") + "Z";
  const bunkers = readFileSync(new URL("../crates/decoder/src/bunkers.rs", import.meta.url), "utf8");
  const dots = [...bunkers.matchAll(/^\s+\[(-?[\d.]+), -?[\d.]+, (-?[\d.]+)\],$/gm)]
    .map((m) => `<circle cx="${+m[1] / 16}" cy="${(MAP - +m[2]) / 16}" r="6" fill="#3ddbd9"/>`)
    .join("");
  writeFileSync(
    values.svg,
    `<svg xmlns="http://www.w3.org/2000/svg" width="1024" height="1024" viewBox="0 0 1024 1024"><rect width="1024" height="1024" fill="#0b0e13"/>` +
      coast.map((l) => `<path d="${path(l)}" fill="#1a212b" stroke="#7d8796" stroke-width="2"/>`).join("") +
      built.map((l) => `<path d="${path(l)}" fill="#2a3340" stroke="#4a5668" stroke-width="1"/>`).join("") +
      dots +
      `</svg>`,
  );
}
