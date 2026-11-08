#!/usr/bin/env node
// yatare demo harness — naive jQuery-era VDOM lives HERE (host side, real
// JS), the numeric layout math lives in compiled bytecode on the yatare VM.
//
// Flow: compile demo/layout.js -> hydrate manifest slots from the VDOM ->
// run -> write results back -> serialize/deserialize the tree through XML
// (.html()-style) -> then optimize (obfuscate mode) and prove the output is
// identical. Edge cases: deliberately ignored, burned with fire.
//
// Usage: node demo/harness.js [--yatare <path>]

"use strict";
const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const argv = process.argv.slice(2);
function argOf(flag, dflt) {
  const i = argv.indexOf(flag);
  return i >= 0 ? argv[i + 1] : dflt;
}
const YATARE = argOf("--yatare", path.join(__dirname, "..", "target", "debug", "yatare"));
const DEMO = path.join(__dirname, "layout.js");
const ASM = path.join(__dirname, "layout.asm");
const OPT_ASM = path.join(__dirname, "layout.opt.asm");

function fail(msg) {
  console.error("FAIL: " + msg);
  process.exit(1);
}
function yatare(args) {
  const r = spawnSync(YATARE, args, { encoding: "utf8" });
  if (r.error) fail(`cannot spawn ${YATARE}: ${r.error.message}`);
  return r;
}

// --- naive doubly-linked VDOM --------------------------------------------
function node(tag, attrs, text) {
  return { tag, attrs: attrs || {}, text: text || "", children: [], parent: null };
}
function append(parent, child) {
  child.parent = parent;
  parent.children.push(child);
  return child;
}
function removeChild(child) {
  const p = child.parent;
  if (!p) return;
  p.children.splice(p.children.indexOf(child), 1);
  child.parent = null;
}

// serialize: element tree -> XML (our own format, our own rules)
function toXml(n) {
  const attrs = Object.entries(n.attrs)
    .map(([k, v]) => ` ${k}="${v}"`)
    .join("");
  if (n.text) return `<${n.tag}${attrs}>${n.text}</${n.tag}>`;
  if (!n.children.length) return `<${n.tag}${attrs}/>`;
  return `<${n.tag}${attrs}>${n.children.map(toXml).join("")}</${n.tag}>`;
}

// deserialize: XML -> element tree. Naive stack machine over OUR OWN output;
// any input this serializer did not produce is out of scope by design.
function fromXml(xml) {
  const re = /<(\w+)((?: \w+="[^"]*")*)\s*(\/)?>|<\/(\w+)>|([^<]+)/g;
  const stack = [];
  let root = null;
  let m;
  while ((m = re.exec(xml))) {
    if (m[1]) {
      const attrs = {};
      const attrRe = /(\w+)="([^"]*)"/g;
      let a;
      while ((a = attrRe.exec(m[2]))) {
        const v = a[2];
        attrs[a[1]] = v !== "" && !isNaN(+v) ? +v : v;
      }
      const n = node(m[1], attrs);
      if (stack.length) append(stack[stack.length - 1], n);
      else if (!root) root = n;
      if (!m[3]) stack.push(n);
    } else if (m[4]) {
      stack.pop();
    } else if (m[5] && m[5].trim()) {
      if (stack.length) stack[stack.length - 1].text = m[5].trim();
    }
  }
  if (stack.length) fail("unbalanced XML (serializer bug?)");
  return root;
}

// --- build the page -------------------------------------------------------
function buildPage(vw) {
  const page = node("page", { viewport: vw });
  append(page, node("title", {}, "Layout demo"));
  append(page, node("banner", { id: "banner", left: 120, top: 40, width: 300, height: 80 }));
  append(page, node("sidebar", { id: "sidebar", left: 40, top: 8 }));
  return page;
}
function nodeFor(page, selector) {
  // selectors are naive: "#name" -> element with id/name
  const id = selector.replace(/^#/, "");
  let found = null;
  (function walk(n) {
    if (n.attrs.id === id || n.tag === id) found = found || n;
    n.children.forEach(walk);
  })(page);
  if (!found) fail(`no VDOM node for selector ${selector}`);
  return found;
}

// --- manifest & running ---------------------------------------------------
function parseManifest(asmText) {
  const out = [];
  const re = /manifest: "([^"]+)" (\w+) -> local\[(\d+)\]/g;
  let m;
  while ((m = re.exec(asmText))) out.push({ selector: m[1], prop: m[2], slot: +m[3] });
  return out;
}
function hydrate(manifest, page) {
  // values must land at their manifest slots, not positionally (16 = N_LOCALS)
  const vals = new Array(16).fill(0);
  for (const e of manifest) vals[e.slot] = nodeFor(page, e.selector).attrs[e.prop];
  return vals;
}
function applyBack(manifest, page, locals) {
  for (const e of manifest) nodeFor(page, e.selector).attrs[e.prop] = locals[e.slot];
}
function runAsm(asmPath, slotValues) {
  const r = yatare(["run", asmPath, "--locals", slotValues.join(",")]);
  if (r.status !== 0) fail(`yatare run ${asmPath} failed:\n${r.stdout}${r.stderr}`);
  const res = /result: (-?\d+)/.exec(r.stdout);
  if (!res) fail(`no result in output: ${r.stdout}`);
  const locals = {};
  const re = /\[(\d+)\]=(-?\d+)/g;
  let m;
  while ((m = re.exec(r.stdout))) locals[+m[1]] = +m[2];
  return { result: +res[1], locals };
}

// --- main -----------------------------------------------------------------
console.log("[1] compile demo/layout.js");
const cr = yatare(["compile", DEMO, "--out", ASM]);
if (cr.status !== 0) fail(`compile failed:\n${cr.stderr}`);
console.log("    " + cr.stderr.trim());
const manifest = parseManifest(fs.readFileSync(ASM, "utf8"));
if (!manifest.length) fail("manifest empty");

const scenarios = [
  { name: "default", banner: { left: 120, width: 300 }, sidebar: { top: 8 },
    expect: { result: 140024, "banner.left": 140, "sidebar.top": 24 } },
  { name: "overflow", banner: { left: 700, width: 300 }, sidebar: { top: -5 },
    expect: { result: 620027, "banner.left": 620, "sidebar.top": 27 } },
];

function runScenario(asmPath, sc) {
  const page = buildPage(960);
  Object.assign(nodeFor(page, "#banner").attrs, sc.banner);
  Object.assign(nodeFor(page, "#sidebar").attrs, sc.sidebar);
  const before = toXml(page);
  const { result, locals } = runAsm(asmPath, hydrate(manifest, page));
  applyBack(manifest, page, locals);
  const after = toXml(page);
  return { before, after, page, result, locals };
}

function checkExpectations(tag, sc, r) {
  const wants = [
    ["result", r.result],
    ["banner.left", nodeFor(r.page, "#banner").attrs.left],
    ["sidebar.top", nodeFor(r.page, "#sidebar").attrs.top],
  ];
  for (const [k, got] of wants) {
    const key = k === "result" ? "result" : k;
    if (sc.expect[key] !== got) {
      fail(`[${tag}/${sc.name}] ${k}: expected ${sc.expect[key]}, got ${got}`);
    }
  }
}

console.log("[2] run scenarios on the original bytecode");
const original = new Map();
for (const sc of scenarios) {
  const r = runScenario(ASM, sc);
  checkExpectations("original", sc, r);
  original.set(sc.name, r);
  console.log(`    ${sc.name}: result=${r.result} banner.left=${nodeFor(r.page, "#banner").attrs.left} sidebar.top=${nodeFor(r.page, "#sidebar").attrs.top}`);
  console.log(`      vdom: ${r.after}`);
}

console.log("[3] XML serialize -> deserialize round-trip (.html()-style)");
const rtPage = original.get("default").page;
const rt = fromXml(toXml(rtPage));
if (toXml(rt) !== toXml(rtPage)) fail("XML round-trip changed the tree");
removeChild(rt.children[rt.children.length - 1]); // exercise removeChild
console.log(`    round-trip stable; tree after removeChild: ${toXml(rt)}`);

console.log("[4] optimize (dense algebraic folding) and prove equivalence");
const or = yatare(["optimize", ASM, "--mode", "obfuscate", "--seed", "5", "--iters", "4000"]);
if (or.status !== 0) fail(`optimize failed:\n${or.stdout}${or.stderr}`);
if (!or.stdout.includes("verification: PASS")) fail("optimizer did not verify");
const lines = or.stdout.split("\n");
const a = lines.findIndex((l) => l.startsWith("--- after"));
const b = lines.findIndex((l, i) => i > a && l.startsWith("length:"));
fs.writeFileSync(OPT_ASM, lines.slice(a + 1, b).join("\n") + "\n");
console.log("    " + lines.slice(b, b + 3).join(" | ").replace(/\n/g, ""));

for (const sc of scenarios) {
  const r = runScenario(OPT_ASM, sc);
  checkExpectations("optimized", sc, r);
  const orig = original.get(sc.name);
  const canon = (o) => JSON.stringify(Object.keys(o).sort().map((k) => [k, o[k]]));
  if (canon(r.locals) !== canon(orig.locals)) {
    fail(`[${sc.name}] optimized locals differ: ${JSON.stringify(r.locals)} vs ${JSON.stringify(orig.locals)}`);
  }
  console.log(`    ${sc.name}: optimized output identical`);
}

console.log("ALL GREEN");
