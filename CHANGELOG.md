# Changelog

Notable changes to this project, per `Keep a Changelog` conventions.
The project has not had a release yet; everything lives under `[Unreleased]`.

## [Unreleased]

### Added

- **14-instruction stack VM** (`src/vm.rs`): one untagged `[i32; 256]` operand
  stack, fixed `[i32; 16]` locals, wrapping i32 arithmetic, explicit control
  flow (`jeq`/`jlt`/`jmp`/`ret`), traps for underflow/overflow/bad local/bad
  pc, and a step budget that turns livelocks into a first-class outcome.
  Zero allocation inside the execution loop (AGENTS.md "Zero Heap Overhead").
- **Text bytecode** (`src/parser.rs`): human-writable assembly with labels and
  `#` comments, two-pass label resolution, canonical disassembly that
  round-trips (pc annotations tolerated), line-numbered errors.
- **Oracle-guided stochastic optimizer** (`src/optimizer.rs`):
  - block-structured mutation space — straight-line blocks with *pinned
    terminators* and block-index jump targets, so insert/delete/transpose can
    never leave a dangling address;
  - CFG-wide stack typing (JVM-verifier style, per-op operand demands);
  - deterministic xorshift64* RNG (zero dependencies), seeded test vectors,
    originals executed once and cached, eager candidate rejection;
  - **three-tier cost function** (user design, supertimizer/obfuscator
    dilemma): oracle hard gate → linear instruction economy (`W_LEN=7`) →
    structural entropy rewarding productive stack juggling and penalizing
    DCE/peephole-trivial idioms — dense algebraic folding, never bloat, with
    a hard obfuscate growth budget of `original + 4`;
  - `--mode size|obfuscate`, fresh-vector final verification (anti-overfit),
    full search statistics in the report.
- **JS-subset front end** (`src/js/`): hand-rolled lexer; precedence-climbing
  parser with C-faithful precedence (`==` binds tighter than `^`); lowering
  with compile-time function inlining (no `CALL` op exists), compile-time
  selector interning, jqLite facade (`$()`, `.offset()`, `.width()`,
  `.height()`, `.css()` chains), handle variables, flat ≤16-slot scoping, and
  a manifest (`# manifest: "<sel>" prop -> local[N]`) that is the harness I/O
  contract. Emitted asm self-validates: it must parse back and pass stack
  typing before `compile` returns.
- **CLI**: `yatare compile`, `yatare run [--budget] [--locals ...]`,
  `yatare disasm`, `yatare optimize [--mode --seed --iters --vectors --verify]`.
- **Demo** (`demo/`): `layout.js` — 2000s-style jqLite layout/offset math with
  inlining, `&&`/`||`/`!`, loops and clamping — plus `harness.js`, a naive
  doubly-linked VDOM in the host with XML serialize/deserialize round-trip,
  manifest-based hydration, and an optimize-then-prove-equivalence pass.
- **Manifesto** (`MANIFESTO.md`): "The Glass Box and the Gravity Well" — the
  project's stance (local-first code self-defense, dense algebraic folding vs
  cloud locks/DRM), the opposition's dilemma and escalation vectors (SMT,
  behavioral cloning, boundary hooking, the LLM ratchet), Phase 2
  counter-moves, and future emitter targets (WASM → JVM/.NET) — linked from
  README's "Why", with counter-measures and targets queued in the TODO
  backlog.
- **Tests**: 69 green (VM semantics, parser round-trips, typing rejections,
  optimizer determinism/search outcomes, JS front-end behavior and error
  classes, CLI end-to-end, full demo) — `cargo test`, plus
  `clippy --all-targets -D warnings` and `cargo fmt --check` gates.
- **Project**: MIT `LICENSE` (Alexey S. Kompaniets), `.gitignore`, task flow
  per AGENTS.md (TODO → SESSION → CHANGELOG).

### Known limitations

- Oracle is property-based, not a proof; verification strength scales with
  `--verify` vector count.
- Mutators are intra-block only: terminator/branch restructuring (deeper
  cross-block folding) is deferred to a later stage.
- Division, floats, strings at runtime, arrays/objects, closures, recursion,
  and callbacks are out of the v0 JS subset by design.
- Forensic watermarking (`watermark.rs`) not yet implemented.
