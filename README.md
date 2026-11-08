# yatare

**Yet Another Transformation Against Reverse Engineering**

`yatare` is a stack-based VM bytecode superoptimizer and obfuscation utility. It takes human-written or compiler-generated basic blocks, establishes an input/output correctness oracle via property-based execution testing, and applies stochastic local search to synthesize an alternative, highly non-idiomatic instruction sequence that is functionally identical but structurally opaque to reverse-engineering tools and LLMs.

## Architecture

1. **Stack-Based VM (`src/vm.rs`):** A lightweight, 14-instruction stack machine featuring fixed local storage, zero hardware registers, and explicit control-flow primitives.
2. **Oracle-Guided Search (`src/optimizer.rs`):** A stochastic mutation engine that evaluates instruction sequences against a test vector suite, filtering out semantic divergence and selecting for instruction reduction or structural variance.
3. **Forensic Watermarking (`src/watermark.rs`):** Optional injection of biased cost functions and algebraic constants to bind ownership profiles directly into the binary layout.

## Installation & Usage

### Build
```bash
cargo build --release
```
