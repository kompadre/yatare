# TODO.md — task board

Legend: `[x]` done · `[~]` in progress · `[ ]` pending. This stage table is
the source of truth for stage state (README mirrors it); conventions live in
`AGENTS.md`.

This file is the **queue**: only tasks not yet started get their breakdown
here. When a task starts, its breakdown and open questions are **evicted into
the current `SESSION.md` entry**; when it completes, detail moves from SESSION
into `CHANGELOG.md` and the stage row below gets ticked.

## Stages

- [ ] PoC implementation including creation of a bespoke VM with 14 unique instructions bytecode interpeter, 
   JS compiler. Compiler takes JS source and transforms it to the custom bytecode, "optimizing" special blocks). 
   - [x] 14-op stack VM + interpreter + asm parser (`src/vm.rs`, `src/parser.rs`)
   - [x] Oracle-guided stochastic optimizer (`src/optimizer.rs`)
   - [ ] JS subset compiler + naive-jQuery/VDOM layout-offset demo (`src/js/`)
- [ ] Full WASM implementation:

## Open design questions (resolve before/during the named stage)

## Backlog (nice to have)


