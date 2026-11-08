# SESSION.md — workbench & handoff log

Newest session first. Read `AGENTS.md` (policy) and `TODO.md` (queue) first.

Role in the task flow: the **current task's detail lives here** (evicted from
`TODO.md` when the task starts) and is **evicted into `CHANGELOG.md` when the
task completes**, leaving a compact outcome line. Finished sessions remain as
chronology and gotchas.

## Session 2026-11-08 — Stage 1 PoC: VM + optimizer + JS pipeline (in progress)

**Task:** Stage 1 from `TODO.md`; breakdown evicted here per AGENTS.md §3.A.
**Bar:** green `cargo test` (+ `fmt`, `clippy -D warnings`) before every commit.
**Scope (raised by user mid-kickoff):** full pipeline today — 14-op VM, asm
parser, oracle-guided optimizer, hand-rolled JS-subset compiler, and a naive
jQuery-style demo on a simplified VDOM (hero workload: **layout/offset math**,
2000s-style chained calls).

### Decisions (resolved design questions)

| Question | Decision |
|---|---|
| Language | Pure Rust, zero external deps; hand-rolled xorshift64 RNG (deterministic seeds per AGENTS.md) |
| Numeric model | i32 wrapping arithmetic; observable = halt result + all 16 locals |
| ISA size | 14 instructions (README said 12; user chose DUP+SWAP+MUL, then added RET) |
| JS front-end | Hand-rolled lexer + Pratt parser for a bounded JS subset |
| Demo | naive jqLite on simplified VDOM; Node harness hydrates/reads VM locals via a compiler-emitted manifest |
| License | MIT added: Alexey S. Kompaniets <kompadre@gmail.com> |
| Git | repo initialized (`main`), `Stage N:` / `Area:` commits with explanatory bodies |

### ISA v0 (14 ops)

| Op | Stack | Semantics |
|---|---|---|
| `PUSH imm` | +1 | i32 constant, wrapping semantics elsewhere |
| `POP` | −1 | discard |
| `LOAD l` / `STORE l` | +1 / −1 | locals `[i32; 16]`, index 0..=15 |
| `ADD` `SUB` `MUL` `XOR` | −1 | wrapping i32 |
| `DUP` | +1 | copy top |
| `SWAP` | 0 | exchange top two |
| `JEQ addr` / `JLT addr` | −2 | pop b, a; jump if a==b / signed a<b |
| `JMP addr` | 0 | unconditional |
| `RET` | −1 | pop result, halt; falling off end halts with top-of-stack (0 if empty) |

Fixed stack `[i32; 256]`, zero allocation in the step loop. No `CALL` op ⇒
source functions are **compile-time inlined**; selectors/classes are
**compile-time interned to i32** (one uniform untagged stack). No AND/OR/NOT ⇒
class-mask logic must be XOR + JLT/JEQ patterns.

### Implementation breakdown (phases)

- [ ] P0 bootstrap: git, cargo scaffold, LICENSE, .gitignore, doc evictions (this entry)
- [ ] P1 `src/vm.rs`: `Instr`, `Vm`, interpreter loop, traps, step budget
- [ ] P2 `src/parser.rs`: asm text ↔ program, labels, round-trip tests
- [ ] P3 oracle (in `src/optimizer.rs`): seeded vectors, cache original runs
- [ ] P4 mutators (insert/delete/transpose + identity pairs), stack-typing validity, fitness (`--mode size|obfuscate`)
- [ ] P5 CLI (`run`, `optimize`), sample programs
- [ ] P6 JS subset compiler (`src/js/`): lexer, Pratt parser, resolver (≤16 locals), codegen with inlining + interning
- [ ] P7 naive-jQuery/VDOM layout-offset demo + harness
- [ ] P8 gates green; doc flow: detail → CHANGELOG `[Unreleased]`, tick TODO sub-items, README usage/architecture update

### Gotchas & constraints (accumulate here)

- **Block-structured search:** program splits into straight-line blocks with
  *pinned terminators*; jump targets are block indices, so intra-block mutation
  can never leave dangling addresses.
- v0 mutators are intra-block only; terminator/branch mutators deferred (the
  ISA has no inverse-branch op — `JLT` inversion needs a synthetic skip block).
- Oracle is property-based, not a proof; final verification uses a much larger
  vector count than in-search fitness. Candidate traps/step-limit ⇒ reject.
- Run each vector against the original **once**, cache results; candidates are
  compared to the cache (search throughput).
- Adding `src/js/` extends the README architecture list (README is agent-editable
  when architecture changes); AGENTS.md stays untouched per Instruction 0.
