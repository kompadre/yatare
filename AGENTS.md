# AGENTS.md — how to work in this repository

Guidance for AI agents (and humans) contributing to **tee-lua-runner**.
Policy and conventions live here; state lives elsewhere (see Task flow).

## Instruction 0 — AGENTS.md is user-owned (read-only for agents)

`AGENTS.md` (this file, the document proper) must not be modified by an
agent. The single exception: the user explicitly prompts for an
addition/modification, **literally describing the change and telling where to
put it** — then the agent applies exactly that description, nothing more.

To reiterate, this rule is only about modification of the `AGENTS.md` file
itself. The other documents of the task flow (`TODO.md`, `SESSION.md`,
`CHANGELOG.md`, `README.md`) remain agent-editable under the rules below.

## What this project is

`yatare` is a stack-based VM bytecode superoptimizer and obfuscation utility. It takes human-written or compiler-generated basic blocks, establishes an input/output correctness oracle via property-based execution testing, and applies stochastic local search to synthesize an alternative, highly non-idiomatic instruction sequence that is functionally identical but structurally opaque to reverse-engineering tools and LLMs.

Read `README.md` for architecture; the rest of the doc stack (`TODO.md`,
`SESSION.md`, `CHANGELOG.md`) is linked from there.

### `AGENTS.md`

```markdown
# Repository Development Guidelines (`yatare`)

This document governs the operational rules, code boundaries, and task tracking protocols for automated agents and developers contributing to the `yatare` codebase.

## 1. Core Engineering Principles
- **Semantic Correctness First:** No optimization or transformation may be committed if it fails even a single test vector in the validation oracle.
- **Deterministic Mutators:** Stochastic search components must utilize controlled RNG seeds where reproducibility is required for debugging.
- **Zero Heap Overhead:** The core VM and interpreter loop must remain lightweight, avoiding dynamic heap allocations during execution loops where possible.

## 2. Module Boundaries
- `src/vm.rs`: Contains the instruction set definitions, state evaluation stack, local table, and execution loop. Do not introduce register models or external memory heaps without architectural review.
- `src/optimizer.rs`: Manages mutation operators (insertion, deletion, transposition) and fitness scoring.
- `src/parser.rs`: Handles parsing and serialization of the text-based bytecode format.

## 3. Task Life Cycle & Commit Protocol
- Split large changes into isolated, atomic commits (e.g., separating VM instruction changes from optimizer heuristic updates).
- Verify all changes locally via `cargo test` before submitting patches or build artifacts.

### 3.A Task flow (which doc when)

Tasks move through a pipeline with **eviction at both transitions**:

```
TODO.md ──(task STARTS: evict breakdown + its open questions)──▶ SESSION.md
SESSION.md ──(task COMPLETES: evict detail)──▶ CHANGELOG.md [Unreleased]
                                        (SESSION keeps a compact outcome/handoff line;
                                         TODO stage row gets ticked)
```

| Doc | Role | When to update |
|-----|------|----------------|
| `AGENTS.md` | Stable policy/conventions (this file) | Per **Instruction 0**: only when the user literally prescribes the change and its placement |
| `TODO.md` | **Queue**: stage table + breakdowns/open-questions of *not yet started* tasks + backlog. Evicted on start | When a task starts (evict → SESSION) or completes (tick stage row) |
| `SESSION.md` | **Workbench + handoff log**: the current task's detail (evicted from TODO), its open questions, stage context, decisions, gotchas, compact outcomes of finished sessions | On task start (adopt detail); on completion (evict detail → CHANGELOG, leave outcome line) |
| `CHANGELOG.md` | **Completed** user-visible history (`Keep a Changelog`, `[Unreleased]`) | When a task completes: receive the evicted detail as entries |
| `README.md` | Human-facing overview | When architecture or workflows change |

**Loop:** read this file → take the next task from `TODO.md` →
1. **evict** that task's breakdown (and its open questions) from `TODO.md`
   into the current `SESSION.md` entry,
2. implement — decisions and gotchas accumulate in that SESSION entry,
3. on completion: **evict** the task detail from `SESSION.md` into
   `CHANGELOG.md` `[Unreleased]`, tick the stage row in `TODO.md`, leave a
   compact outcome + next-step line in `SESSION.md`,
4. run the gates (below) — all green — then commit.

Finished sessions stay as chronology (handoff context, gotchas); their
feature-level detail belongs to CHANGELOG only.

Commit message style: `Area: concise summary` or `Stage N: …` with a body
explaining *why*, referencing issue-worthy facts (measured numbers, discoveries).

## Guardrails

- Do not commit guest `*.wasm` build outputs (gitignored), `target/`, or
  toolchain upgrades without an explicit decision.
- Do not weaken CI gates or disable clippy lints to get green; fix the code.
- Ask before: renaming the repo, changing the license posture, bumping the
  pinned toolchain, adding heavyweight dependencies.
