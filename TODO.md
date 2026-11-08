# TODO.md — task board

Legend: `[x]` done · `[~]` in progress · `[ ]` pending. This stage table is
the source of truth for stage state (README mirrors it); conventions live in
`AGENTS.md`.

This file is the **queue**: only tasks not yet started get their breakdown
here. When a task starts, its breakdown and open questions are **evicted into
the current `SESSION.md` entry**; when it completes, detail moves from SESSION
into `CHANGELOG.md` and the stage row below gets ticked.

## Stages

- [x] PoC implementation including creation of a bespoke VM with 14 unique instructions bytecode interpeter, 
   JS compiler. Compiler takes JS source and transforms it to the custom bytecode, "optimizing" special blocks). 
   - [x] 14-op stack VM + interpreter + asm parser (`src/vm.rs`, `src/parser.rs`)
   - [x] Oracle-guided stochastic optimizer (`src/optimizer.rs`)
   - [x] JS subset compiler (`src/js/`: lexer, Pratt parser, inlining codegen, element manifest, self-validation)
   - [x] naive-jQuery/VDOM layout-offset demo + Node harness (`demo/`)
- [ ] Full WASM implementation:

## Open design questions (resolve before/during the named stage)

## Backlog (nice to have)

Speculative defenses — the "drawbridge" counter-measures from
[`MANIFESTO.md`](MANIFESTO.md) (stage 2+; none started, none validated):

- Dynamic control-flow flattening + runtime-seeded opaque predicates
  (vs SMT/Z3 symbolic execution)
- Per-build slot permutation and re-optimization from a fresh seed — v1.0
  surrogates must fail on v1.1 (vs behavioral cloning / the LLM ratchet)
- Manifest slots derived mathematically from prior steps instead of static
  slots; rolling-state encoding of values crossing the manifest interface
  (vs boundary hooking)
- Adversary-in-the-loop fitness: score candidates by a bounded solver's
  failure to recover the formula. *Likely needs an SMT dependency — ask
  before adding (guardrail).*
- `watermark.rs` forensic watermarking (README lists it as planned)
- Terminator/branch mutators (cross-block folding) + dataflow-aware entropy
  scoring for dead-read detection


