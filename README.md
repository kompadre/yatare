# yatare

**Yet Another Transformation Against Reverse Engineering**

`yatare` is a stack-based VM bytecode superoptimizer and obfuscation utility. It takes human-written or compiler-generated basic blocks, establishes an input/output correctness oracle via property-based execution testing, and applies stochastic local search to synthesize an alternative, highly non-idiomatic instruction sequence that is functionally identical but structurally opaque to reverse-engineering tools and LLMs.

Obfuscation here is **dense algebraic folding**, not bloat: the search is
charged per instruction and rewarded for productive stack juggling, so the
result stays short and fast while every instruction remains mathematically
live — dead-code elimination finds nothing to remove.

## Why

Indie logic shouldn't ship as a naked glass box, and the traditional answers —
always-online telemetry, heavyweight DRM — betray the ethos in the opposite
direction. `yatare` is the stubborn, local-first third option: honest about
its odds, ruthless about semantics, local by design.

Read **[The Glass Box and the Gravity Well](MANIFESTO.md)** — the manifesto:
what shipped code deserves, what opposition actually throws at it (SMT solvers,
behavioral cloning, boundary hooking, the LLM ratchet), and the Phase 2
drawbridge for each.

## Architecture

1. **Stack-Based VM (`src/vm.rs`):** A lightweight, 14-instruction stack machine featuring fixed local storage, zero hardware registers, and explicit control-flow primitives. Wrapping i32 semantics, `[i32; 16]` locals, zero allocation in the step loop.
2. **JS front-end (`src/js/`):** Hand-rolled lexer + precedence-climbing parser for a bounded JS subset, lowered to bytecode with compile-time function inlining (there is no `CALL` instruction) and compile-time selector interning (there are no runtime strings). Emits an element-slot manifest that doubles as the host harness's I/O contract, and self-validates its output through the asm parser and stack typing.
3. **Text bytecode (`src/parser.rs`):** Human-writable assembly with labels; canonical disassembly round-trips.
4. **Oracle-Guided Search (`src/optimizer.rs`):** A stochastic mutation engine (insert/delete/transpose inside terminator-pinned blocks) that evaluates candidates against a seeded test-vector suite. Three-tier cost: semantic correctness is a hard gate, shorter is better, structural entropy rewards non-idiomatic transitions — the balance that keeps results dense instead of boring or bloated.
5. **Forensic Watermarking (`src/watermark.rs`):** *Planned* — biased cost functions and algebraic constants to bind ownership profiles into the binary layout.

## Installation & Usage

### Build
```bash
cargo build --release
```

### Compile JS to bytecode
```bash
cargo run -- compile demo/layout.js --out demo/layout.asm
```

### Interpret
```bash
cargo run -- run samples/sum_loop.asm
cargo run -- run demo/layout.asm --locals 120,0,300,0,0,0,0,8,0
```
`--locals` hydrates the initial local slots (the manifest in the compiled asm
says which slot holds which element property).

### Disassemble
```bash
cargo run -- disasm samples/dead_code.asm
```

### Optimize / obfuscate
```bash
cargo run -- optimize samples/dead_code.asm --mode size       # superoptimize
cargo run -- optimize samples/dead_code.asm --mode obfuscate  # short + alien
```
The report shows before/after listings, length and score deltas, search
statistics, and a PASS/FAIL verification against fresh test vectors
(exit code 3 on failure).

### The demo: naive jqLite on a virtual DOM
```bash
cargo build
node demo/harness.js            # or: cargo test --test demo
```
`demo/layout.js` is 2000s-style jQuery-flavored layout math
(`$("#banner").offset().left + 20`, clamping, while-loops). The Node harness
owns a naive doubly-linked VDOM, serializes it to XML and back
(`.html()`-style), hydrates the manifest slots, runs the compiled bytecode,
writes the results into the tree, then obfuscates the bytecode and proves the
outputs are identical.

## Testing

```bash
cargo test                                  # unit + integration + demo
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

All stochastic components take explicit seeds; same seed ⇒ identical report.

## Documentation

Development flow and conventions live in [`AGENTS.md`](AGENTS.md); the task
board, current session, and history live in [`TODO.md`](TODO.md),
[`SESSION.md`](SESSION.md), and [`CHANGELOG.md`](CHANGELOG.md).
