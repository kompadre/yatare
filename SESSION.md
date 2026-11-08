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
| Fitness (corrected same-day, user design: supopt/obfuscator dilemma) | three-tier cost: **oracle hard gate → linear length economy (`W_LEN=7`/insn) → structural entropy** (reward stack-juggling windows, penalize DCE/peephole-trivial idioms). Obfuscate growth budget = original + 4 (hard cap). Dense algebraic folding, never bloat |
| VDOM depth (P7) | naive doubly-linked node tree lives **in the Node harness**; bytecode sees flattened numeric slots via the manifest only (no memory ops in the ISA — extending it would be the real work-loss scenario); XML (de)serialization for `.html('')` in harness JS; edge cases explicitly out of scope |

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

- [x] P0 bootstrap: git, cargo scaffold, LICENSE, .gitignore, doc evictions (this entry)
- [x] P1 `src/vm.rs`: `Instr`, `Vm`, interpreter loop, traps, step budget
- [x] P2 `src/parser.rs`: asm text ↔ program, labels, round-trip tests
- [x] P3 oracle (in `src/optimizer.rs`): seeded vectors, cache original runs
- [x] P4 mutators (insert/delete/transpose + identity pairs), stack-typing validity, fitness (`--mode size|obfuscate`)
- [x] P5 CLI (`run`, `disasm`, `optimize`), sample programs + `tests/cli.rs`
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

### Gotchas learned in P0–P5 (2026-11-08)

- **All 16 locals are observable ⇒ no such thing as a dead write.** A
  `push k / store <never-loaded slot>` unit always diverges (locals differ);
  it was removed from the insert payload. Relaxing to a live-out set is a
  future optimizer upgrade.
- **Fixture rule: never `store` into the input slot.** The first obfuscate
  fixture did `push 0; store 0`, overwriting its own input → every vector
  livelocked → `NoValidVectors`. The oracle caught it; the fixture was rebuilt.
- **Mutator range guards:** `delete_range_max` must be `None` when the cap is
  0 (terminator-only blocks) or `rng.below(0)` panics. All three mutators
  validate position ranges up front.
- **Identity units are depth-sensitive** (`[swap,swap]` needs depth ≥ 2): the
  typecheck — not the unit table — decides feasibility; tests must insert at a
  feasible depth.
- **Verification vector counts can be < requested:** vectors where the
  *original* exceeds the step budget (huge random loop bounds) are excluded by
  design (e.g. 410/512 on `sum_loop`).
- ~~Fitness quirk: adjacent `swap; swap` never penalized~~ — **superseded by
  the same-day fitness correction** (see Decisions); see the rework gotchas
  below.
- Measured P4 behavior (seed 7, size mode on `dead_code.asm`): 14 → 6
  instructions, 6000 iterations = 1491 untypable + 2972 divergent + 110
  accepted, PASS on 512 fresh vectors. Determinism test locks same-seed output.

### Fitness rework gotchas (user correction: "dense algebraic folding", same day)

- **The old `Mode::Obfuscate` was the bloat anti-pattern** (+8/insn for
  dup/swap/xor, growth-rewarding → 20→144 demos) — trivially vaporizable by
  DCE/peephole. Replaced: linear `W_LEN=7`/insn charge + window
  bonuses (dup+add15, swap 10, dup+swap 8, load+dup 5) / penalties
  (swap-swap, dup-pop, push-pop −15; store-forwarding −12; const-fold −10;
  3-gram no-op [dup,swap,pop] −15). Dead-code sample under new scoring:
  14 → **8** (−42.9%), swap-reordered xor, PASS 512 vectors.
- **Length ratchet found while tuning:** neutral noise units net exactly 0 at
  `W_LEN=6`, so plateau-walk accumulated length and small window bonuses then
  sanctified it (18 → 33 observed). Fixes: `W_LEN=7` (units strictly
  negative), explicit 3-gram no-op penalty, hard obfuscate cap `original+4`.
  Never trust a fitness term whose growth charge can be cancelled by its own
  recurring bonus.
- **Typecheck soundness bug (fixed here):** net-effect-only depth tracking
  under-counted operand demands — `add` nets −1 but pops 2, `dup` nets +1 but
  needs 1, `swap` nets 0 but needs 2. The oracle always caught it during
  search, but compile-time `check_program` has **no oracle behind it** → a JS
  program could have compiled, typechecked, then trapped on any run. Fixed
  with per-op `ins_operands` demand (`req = max(demand − depth_so_far)`);
  regression test `typecheck_accounts_for_op_operand_demands`.
- **Verified-equivalence is stronger than it looks:** the search found a
  candidate storing a constant into `local2` on early loop iterations — legal
  because only the *final* state is observable (later iterations overwrite).
  Full-final-state observation is what forces joint folding of outputs;
  do not weaken it to return-value-only.
- **Future scoring refinement (backlog):** dead *reads* with no consumer
  (e.g. `load 9` immediately before a value-producing `ret`) are not
  window-detectable — needs dataflow, not 2-grams. Linear length penalty keeps
  them bounded meanwhile.
