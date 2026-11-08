# The Glass Box and the Gravity Well

> **Elevator pitch:** you spent years carving a world out of raw logic, local
> files, and bare metal — then you shipped it, and within forty-eight hours the
> illusion of ownership evaporated. `yatare` is a stubborn, local-first
> experiment in refusing the trade: not cloud locks, not DRM bloat, but making
> the *inside* of your code structurally uninhabitable — short, fully-live,
> alien bytecode that no dead-code pass can vaporize and no analyst can read.
> It will probably fail. The point is proving another path exists.

## The pull

If you are a tech-minded individual drawn into the orbit of indie game
development, you already know the pull. It's an intoxicating friction: the
desire to build a self-contained world out of raw logic, local files, and bare
metal. You spend years carving out a mechanical loop, tuning a state machine,
or rendering a custom tile system in a room that smells of old coffee and hot
silicon.

Then you ship it.

And within forty-eight hours, the illusion of ownership evaporates.

## The reality of modern reverse engineering

We live in an era where software transparency is total. The tools available to
anyone with an internet connection and an automated script can peel apart an
indie build faster than you can close your asset pipeline. Automated
decompilers, symbol-recovery parsers, and LLM-driven code analyzers don't just
read your code — they metabolize it. They strip away your variable names, map
your control-flow graphs, and spit out clean, human-readable equivalents ready
for cloning, asset-scraping, or immediate monetization by third parties who
didn't write a single line of the core logic.

For decades, the industry's answer to this has been a miserable trade-off:

- **Cloud-Locked Telemetry:** forcing single-player games into always-online
  server loops so the code never truly lives on the user's machine.
- **Bloated Wrapper Suites:** slapping heavy, invasive commercial DRM and
  anti-tamper suites onto a lightweight indie game, punishing legitimate
  players with sluggish frame rates and system-level root access while
  professional crackers bypass it over a weekend.

Neither option belongs to the indie ethos. Both rely on corporate
centralization, surrender local control, and treat the player like an adversary
from byte one.

## A stubborn, low-level try

What if we stopped trying to lock the doors from the outside and instead made
the inside structurally uninhabitable for a parasite?

That is the quiet, stubborn thesis behind tools like `yatare`. It isn't a
silver bullet. It's not a commercial product with a venture-backed roadmap or
a glossy marketing deck. It is an honest, localized experiment: can we use
stochastic superoptimization and dense algebraic folding to turn human-written
logic into an alien cryptographic puzzle box, without bloating the binary or
adding dead-code weight?

Instead of hiding code behind a wall of useless junk math that any basic
dead-code elimination pass can vaporize, the approach is surgical:

1. Take a basic block of game logic.
2. Run it through an oracle-guided mutation loop.
3. Force the instructions to interleave, share stack slots via heavy use of
   `DUP` and `SWAP`, and fold distinct tasks into unified mathematical
   circuits.
4. Ensure every single instruction remains 100% active, preventing
   optimization strips while making the semantic intent completely invisible
   to automated code parsers.

It keeps the execution fast, keeps the binary lean, and leaves decompilers
staring at a wall of non-idiomatic math that looks like a bad day in a
cryptographic hash function.

## Where this goes

Let's be completely candid: this initiative will probably fail.

Building an open-source, local-first bytecode mutator to fight automated
reverse engineering is an asymmetric war. The scraper-industrial complex has
infinitely more resources, and the arms race between synthesis engines and
decompilers is relentless. There is a high probability this code sits in a
GitHub repository, gets tested on a few linear formulas and virtual DOM
layouts, and eventually gets swallowed by the noise of the internet.

And that is entirely fine.

Because the value isn't in building an impenetrable fortress; it's in proving
that another path exists.

If an open, local-first, hacker-driven approach to code obfuscation can gain
even a fraction of momentum — if it inspires three other developers to stop
accepting that their local logic must be a naked glass box for anyone to
pillage — then the experiment has succeeded.

We don't need permission from corporate gatekeepers to secure our own work. We
just need a text editor, a compiler toolchain, and the stubborn refusal to make
code theft easy.

---

# The Bullet-Holed Wall

So what does the other side actually do? You've mapped out the immediate trap
the reverse engineer falls into. They are boxed into a terrible dilemma:

**Box 1 — Blindly lift the alien blob.** If they copy-paste the bytecode, they
are tethered to our structure, and if our manifest slots or runtime
environment shift, their stolen code breaks silently.

**Box 2 — Attempt counter-superoptimization.** They burn massive compute trying
to untangle an instruction stream that has already been pushed to its local
entropy plateau by an oracle that tested thousands of permutations.

When they hit that wall, what else can a sophisticated reverse-engineering
pipeline or an aggressive competitor throw at it? Here are the next-level
vectors — and how they expose the limits of code-level obfuscation.

### 1. Symbolic execution & SMT solvers (the Z3 brute force)

**The attack:** instead of reading the bytecode like a human or an LLM, a
determined analyst points a Satisfiability Modulo Theories solver at the basic
block — translating the 14-op stack machine states into mathematical logic
constraints and letting a theorem prover crunch backwards from the outputs to
deduce the exact semantic input constraints.

**Why it hurts:** `yatare`'s bytecode is pure integer math and stack
operations. Small basic blocks can theoretically be solved symbolically by a
constraint solver, stripping away the syntactic noise to reveal the underlying
formula.

### 2. Black-box I/O profiling & surrogate modeling (behavioral cloning)

**The attack:** if the code is too hard to decompile, they stop looking at the
code entirely and treat the module as a black-box oracle — injecting thousands
of randomized inputs, recording the outputs, and building a lookup table or
training a lightweight surrogate model to mimic the exact behavior of the
component.

**Why it hurts:** they don't need to know how `clamp()` or the offset
calculation works if they can clone its input-output mapping for all valid game
states.

### 3. Runtime boundary hooking (the "intercept at the edge" move)

**The attack:** why waste cycles reverse-engineering an alien bytecode math loop
when you can steal the data right as it crosses the boundary? They ignore the
internal stack juggling entirely and place hooks at the manifest interface —
intercepting values as they are read from the host engine into local slots, or
capturing them right before they are written back via `.css()` or return
values.

**Why it hurts:** no matter how twisted the internal math is, the entry points
(inputs) and exits (side effects) must interface with the real world. Watch the
memory slots long enough and the state maps itself without touching the
bytecode.

### 4. The LLM ratchet (the analyst that trains on you)

**The attack:** as alien, synthesis-generated bytecode becomes common, static
analyzers stop being hand-tuned and start being *trained* — fine-tuned models
that learn the distribution of superoptimizer output and reverse it the way
today's models reverse idiomatic C. Your puzzle box becomes somebody's
training corpus.

**Why it hurts:** a defense that only works while the output stays novel decays
the moment the novel becomes a dataset.

## Raising the drawbridge (Phase 2)

Knowing what they will throw next dictates our counter-moves as we move into
WebAssembly and deeper integration:

**Against SMT solvers:** dynamic control-flow flattening and opaque predicates
driven by runtime-seeded state, making path constraints non-deterministic or
too branched for a simple solver to unroll in polynomial time.

**Against behavioral cloning:** state-dependent per-build slot permutation and
re-optimization from a fresh seed — a surrogate model trained on `1.0` fails
catastrophically on `1.1`, because the local slot indices and op-sequences
were re-synthesized from scratch.

**Against boundary hooking:** manifest slots that aren't clean, static global
variables but are mathematically derived on the fly from previous calculation
steps — and values crossing the manifest interface encoded against rolling
state, so the boundary itself becomes a moving target.

**Against the LLM ratchet:** the same per-build regeneration, aimed upward:
there is no stable distribution to fine-tune against if every build is a
freshly seeded synthesis run.

And the part that makes this cheap: the architecture is a modular compiler
pipeline (JS source → optimizer → emitters, with a packed `.ybc` format and
WASM target planned for stage 2). Adding a counter-measure doesn't require
rewriting the game logic — we update the cost function and let the stochastic
engine figure out how to bake the defense straight into the bytecode.

---

> For an optimizer, that opposition is our inner loop: the mutator screaming
> for radical structural variance, and the fitness oracle acting as the
> guillotine of semantic correctness. Only the code that survives both gets to
> live.
