//! Oracle-guided stochastic local search (AGENTS.md: deterministic mutators,
//! controlled RNG seeds).
//!
//! Pipeline: flat program -> [`Ir`] (straight-line blocks with *pinned*
//! terminators; jump operands become block indices, so intra-block mutation
//! can never leave a dangling address) -> stack-typing check (CFG-wide depth
//! fixed point, JVM-verifier style) -> seeded property-based oracle (original
//! executed once per vector, candidates compared against the cache) ->
//! fitness-scored acceptance (simulated-annealing-lite).

use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt;

use crate::vm::{DEFAULT_BUDGET, Instr, N_LOCALS, Outcome, exec};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Superoptimization: minimize instruction count.
    Size,
    /// Obfuscation: select for non-idiomatic structure (redundancy, stack
    /// juggling, xor density), penalize recognizable peephole idioms.
    Obfuscate,
}

#[derive(Clone, Debug)]
pub struct Config {
    pub mode: Mode,
    pub seed: u64,
    pub iterations: u64,
    pub search_vectors: usize,
    pub verify_vectors: usize,
    pub budget: u64,
}

impl Config {
    pub fn new(mode: Mode) -> Self {
        Self {
            mode,
            seed: 42,
            iterations: 6_000,
            search_vectors: 64,
            verify_vectors: 512,
            budget: DEFAULT_BUDGET,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub mode: Mode,
    pub seed: u64,
    pub original_len: usize,
    pub candidate_len: usize,
    pub iterations: u64,
    pub proposed: u64,
    pub rejected_untypable: u64,
    pub rejected_divergent: u64,
    pub accepted: u64,
    pub verified: bool,
    pub verify_vectors_used: usize,
    pub original: Vec<Instr>,
    pub candidate: Vec<Instr>,
}

#[derive(Debug)]
pub enum OptError {
    UntypableOriginal(String),
    NoValidVectors,
}

impl fmt::Display for OptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptError::UntypableOriginal(s) => {
                write!(f, "original program fails stack typing: {s}")
            }
            OptError::NoValidVectors => {
                write!(f, "original program does not halt on any test vector")
            }
        }
    }
}

impl std::error::Error for OptError {}

/// Search for an oracle-equivalent program. The returned report's candidate
/// is re-verified against a *fresh* vector set (different seed stream) before
/// `verified` is set.
pub fn optimize(prog: &[Instr], cfg: &Config) -> Result<Report, OptError> {
    let ir = Ir::split(prog).map_err(OptError::UntypableOriginal)?;
    ir.typecheck().map_err(OptError::UntypableOriginal)?;

    let oracle = Oracle::new(prog, cfg.search_vectors, cfg.seed, cfg.budget);
    if oracle.used() == 0 {
        return Err(OptError::NoValidVectors);
    }

    let mut rng = Rng::new(cfg.seed);

    let mut current = ir;
    let mut cur_score = score(cfg.mode, &current.flatten());
    let mut best = current.clone();
    let mut best_score = cur_score;

    let mut proposed = 0u64;
    let mut rejected_untypable = 0u64;
    let mut rejected_divergent = 0u64;
    let mut accepted = 0u64;

    let cap = prog.len() * 4 + 64;

    for _ in 0..cfg.iterations {
        let mut cand = current.clone();
        if !cand.mutate(&mut rng) {
            continue;
        }
        proposed += 1;
        if cand.total_len() > cap {
            continue;
        }
        if cand.typecheck().is_err() {
            rejected_untypable += 1;
            continue;
        }
        let flat = cand.flatten();
        if !oracle.consistent(&flat) {
            rejected_divergent += 1;
            continue;
        }
        let s = score(cfg.mode, &flat);
        let take = if s > cur_score {
            true
        } else if s == cur_score {
            rng.chance(1, 10) // plateau walk
        } else {
            rng.chance(3, 100) // escape local optimum
        };
        if take {
            accepted += 1;
            cur_score = s;
            current = cand;
            if s > best_score {
                best_score = s;
                best = current.clone();
            }
        }
    }

    // Fresh, independent verification stream (anti-overfitting).
    let fresh = Oracle::new(
        prog,
        cfg.verify_vectors,
        cfg.seed ^ 0x9e37_79b9_7f4a_7c15,
        cfg.budget,
    );
    let candidate = best.flatten();
    let verified = fresh.used() > 0 && fresh.consistent(&candidate);

    Ok(Report {
        mode: cfg.mode,
        seed: cfg.seed,
        original_len: prog.len(),
        candidate_len: candidate.len(),
        iterations: cfg.iterations,
        proposed,
        rejected_untypable,
        rejected_divergent,
        accepted,
        verified,
        verify_vectors_used: fresh.used(),
        original: prog.to_vec(),
        candidate,
    })
}

// ---------------------------------------------------------------------------
// Deterministic RNG: xorshift64* (no external crates per repo guardrails)
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    /// Uniform in `0..n` (n > 0).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }

    fn chance(&mut self, num: u64, den: u64) -> bool {
        self.below(den as usize) < num as usize
    }

    fn magic_i32(&mut self) -> i32 {
        const MAGIC: [i32; 9] = [
            0,
            1,
            -1,
            2,
            -2,
            0x5a5a_5a5a,
            0x00ff_00ff,
            i32::MAX,
            i32::MIN,
        ];
        match self.below(MAGIC.len() + 1) {
            i if i == MAGIC.len() => self.next_u64() as i32,
            i => MAGIC[i],
        }
    }

    fn any_i32(&mut self) -> i32 {
        self.next_u64() as i32
    }

    fn gen_vector(&mut self) -> [i32; N_LOCALS] {
        let mut v = [0i32; N_LOCALS];
        for slot in &mut v {
            *slot = match self.below(4) {
                0 => self.any_i32(),
                1 | 2 => self.below(33) as i32 - 16, // small values
                _ => self.magic_i32(),
            };
        }
        v
    }
}

// ---------------------------------------------------------------------------
// IR: straight-line blocks, block-index jump targets, pinned terminators
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Block {
    instrs: Vec<Instr>, // jump operands are BLOCK indices here
}

#[derive(Clone)]
struct Ir {
    blocks: Vec<Block>, // the "sink" (halt) is index blocks.len()
}

fn ins_effect(ins: &Instr) -> i32 {
    match ins {
        Instr::Push(_) | Instr::Load(_) | Instr::Dup => 1,
        Instr::Pop | Instr::Store(_) | Instr::Add | Instr::Sub | Instr::Mul | Instr::Xor => -1,
        Instr::Swap => 0,
        Instr::Jeq(_) | Instr::Jlt(_) => -2,
        Instr::Jmp(_) => 0,
        Instr::Ret => -1,
    }
}

impl Ir {
    /// Split a flat program into blocks. Split points: program start, every
    /// jump target, every instruction after a terminator, and the end. This
    /// guarantees every jump target is a block start.
    fn split(prog: &[Instr]) -> Result<Ir, String> {
        let len = prog.len();
        let mut points: BTreeSet<usize> = BTreeSet::new();
        points.insert(0);
        points.insert(len);
        for (i, ins) in prog.iter().enumerate() {
            if ins.is_terminator() && i + 1 < len {
                points.insert(i + 1);
            }
            match ins {
                Instr::Jeq(t) | Instr::Jlt(t) | Instr::Jmp(t) => {
                    if *t > len {
                        return Err(format!("jump target {t} out of range (program len {len})"));
                    }
                    if *t < len {
                        points.insert(*t);
                    }
                }
                _ => {}
            }
        }

        let pts: Vec<usize> = points.iter().copied().collect();
        let mut blocks = Vec::new();
        for w in pts.windows(2) {
            if w[0] < w[1] {
                blocks.push(Block {
                    instrs: prog[w[0]..w[1]].to_vec(),
                });
            }
        }

        // Remap jump operands from flat instruction index to block index.
        // t == len maps to the sink (blocks.len()); t < len is a point, and
        // segment j starts at pts[j].
        let n = blocks.len();
        for b in blocks.iter_mut() {
            for ins in b.instrs.iter_mut() {
                let target = match ins {
                    Instr::Jeq(t) | Instr::Jlt(t) | Instr::Jmp(t) => t,
                    _ => continue,
                };
                *target = if *target == len {
                    n
                } else {
                    pts.binary_search(target).map_err(|_| {
                        format!("internal: jump target {target} is not a block start")
                    })?
                };
            }
        }
        Ok(Ir { blocks })
    }

    /// Back to a flat program; block-index targets become instruction indices.
    fn flatten(&self) -> Vec<Instr> {
        let n = self.blocks.len();
        let mut prefix = Vec::with_capacity(n + 1);
        let mut acc = 0usize;
        for b in &self.blocks {
            prefix.push(acc);
            acc += b.instrs.len();
        }
        prefix.push(acc); // prefix[n] == sink == one past the end

        let mut out = Vec::with_capacity(acc);
        for b in &self.blocks {
            for ins in &b.instrs {
                out.push(match *ins {
                    Instr::Jeq(t) => Instr::Jeq(prefix[t]),
                    Instr::Jlt(t) => Instr::Jlt(prefix[t]),
                    Instr::Jmp(t) => Instr::Jmp(prefix[t]),
                    other => other,
                });
            }
        }
        out
    }

    fn total_len(&self) -> usize {
        self.blocks.iter().map(|b| b.instrs.len()).sum()
    }

    /// CFG-wide stack-depth fixed point. Entry depth of block 0 must be 0;
    /// every reachable path must satisfy each block's depth requirement and
    /// agree on successor entry depths. The sink is unconstrained (halting
    /// with different stack heights is observably identical).
    fn typecheck(&self) -> Result<(), String> {
        let n = self.blocks.len();
        if n == 0 {
            return Ok(());
        }

        let mut delta = vec![0i32; n];
        let mut need = vec![0i32; n];
        for (i, b) in self.blocks.iter().enumerate() {
            let mut bal = 0i32;
            let mut minbal = 0i32;
            for (pos, ins) in b.instrs.iter().enumerate() {
                if ins.is_terminator() && pos + 1 != b.instrs.len() {
                    return Err(format!("internal: terminator mid-block in block {i}"));
                }
                bal += ins_effect(ins);
                minbal = minbal.min(bal);
            }
            delta[i] = bal;
            need[i] = -minbal;
        }

        let mut depth: Vec<Option<i32>> = vec![None; n + 1]; // [n] = sink (unused)
        depth[0] = Some(0);
        let mut work = VecDeque::from([0usize]);
        while let Some(b) = work.pop_front() {
            let d = depth[b].expect("reachable depth assigned");
            if d < need[b] {
                return Err(format!(
                    "block {b}: entry depth {d} below requirement {} (stack underflow)",
                    need[b]
                ));
            }
            let instrs = &self.blocks[b].instrs;
            let exit = d + delta[b];
            let mut targets = Vec::new();
            match instrs.last() {
                Some(Instr::Jeq(t)) | Some(Instr::Jlt(t)) => {
                    targets.push(*t);
                    targets.push(b + 1); // fallthrough
                }
                Some(Instr::Jmp(t)) => targets.push(*t),
                Some(Instr::Ret) => {}
                // straight-line end (or empty block): fall through
                _ => targets.push(b + 1),
            }
            for s in targets {
                if s > n {
                    return Err(format!("block {b}: jump target {s} out of range"));
                }
                if s == n {
                    continue; // sink: unconstrained
                }
                match depth[s] {
                    None => {
                        depth[s] = Some(exit);
                        work.push_back(s);
                    }
                    Some(existing) if existing != exit => {
                        return Err(format!(
                            "block {s}: depth conflict {existing} vs {exit} from block {b}"
                        ));
                    }
                    Some(_) => {}
                }
            }
        }
        Ok(())
    }

    /// Apply one random mutation. Returns false when nothing was mutable.
    /// Terminators are never edited, moved, or deleted; jump operands are
    /// block indices and are therefore never touched by any mutation.
    fn mutate(&mut self, rng: &mut Rng) -> bool {
        let n = self.blocks.len();
        if n == 0 {
            return false;
        }
        let kind = rng.below(100);
        match kind {
            0..=44 => {
                // insertion
                let Some(bi) = self.pick_block(rng, |b| b.insert_pos_max().is_some()) else {
                    return false;
                };
                let b = &mut self.blocks[bi];
                let pos = rng.below(b.insert_pos_max().expect("filtered") + 1);
                let payload = insert_payload(rng);
                for (k, ins) in payload.into_iter().enumerate() {
                    b.instrs.insert(pos + k, ins);
                }
                true
            }
            45..=79 => {
                // deletion (contiguous range, never the terminator)
                let Some(bi) = self.pick_block(rng, |b| b.delete_range_max().is_some()) else {
                    return false;
                };
                let b = &mut self.blocks[bi];
                let max_b = b.delete_range_max().expect("filtered"); // exclusive end cap
                let a = rng.below(max_b);
                let b_end = (a + 1 + rng.below(6)).min(max_b);
                b.instrs.drain(a..b_end);
                true
            }
            _ => {
                // transposition of adjacent non-terminator instructions
                let Some(bi) = self.pick_block(rng, |b| b.transpose_pos_max().is_some()) else {
                    return false;
                };
                let b = &mut self.blocks[bi];
                let pos = rng.below(b.transpose_pos_max().expect("filtered") + 1);
                b.instrs.swap(pos, pos + 1);
                true
            }
        }
    }

    fn pick_block(&self, rng: &mut Rng, ok: impl Fn(&Block) -> bool) -> Option<usize> {
        let candidates: Vec<usize> = (0..self.blocks.len())
            .filter(|&i| ok(&self.blocks[i]))
            .collect();
        if candidates.is_empty() {
            None
        } else {
            Some(candidates[rng.below(candidates.len())])
        }
    }
}

impl Block {
    /// Max insert position (inclusive): just before a pinned terminator, or
    /// at the end when there is none. `None` when the block is empty.
    fn insert_pos_max(&self) -> Option<usize> {
        if self.instrs.is_empty() {
            return None;
        }
        Some(if self.instrs.last().is_some_and(Instr::is_terminator) {
            self.instrs.len() - 1
        } else {
            self.instrs.len()
        })
    }

    /// Exclusive end cap for deletion ranges (the terminator is excluded).
    /// `None` when there is nothing deletable (cap 0 = terminator-only block).
    fn delete_range_max(&self) -> Option<usize> {
        if self.instrs.is_empty() {
            return None;
        }
        let cap = if self.instrs.last().is_some_and(Instr::is_terminator) {
            self.instrs.len() - 1
        } else {
            self.instrs.len()
        };
        (cap > 0).then_some(cap)
    }

    /// Max lower index of an adjacent swap; both sides must precede any
    /// pinned terminator.
    fn transpose_pos_max(&self) -> Option<usize> {
        let cap = self.delete_range_max()?; // terminator index (or len)
        if cap >= 2 {
            Some(cap - 2) // pair (pos, pos+1) with pos+1 <= cap-1
        } else {
            None
        }
    }
}

/// Payload for an insertion. Terminators are never generated (they would
/// become mid-block); control flow is immutable in this mutation set.
///
/// Identity units are stack-depth-neutral but not all depth-feasible
/// everywhere (e.g. `[swap, swap]` needs depth >= 2) — the typecheck decides
/// feasibility, the oracle decides semantic neutrality. Note: every local is
/// part of the observable state, so there is no such thing as a dead write.
fn insert_payload(rng: &mut Rng) -> Vec<Instr> {
    match rng.below(100) {
        // 50%: balanced identity unit (always typechecks, mostly oracle-safe)
        0..=49 => {
            let k = rng.magic_i32();
            match rng.below(8) {
                0 => vec![Instr::Push(k), Instr::Pop],
                1 => vec![Instr::Dup, Instr::Pop],
                2 => vec![Instr::Swap, Instr::Swap],
                3 => vec![Instr::Push(k), Instr::Xor, Instr::Push(k), Instr::Xor],
                4 => vec![Instr::Push(k), Instr::Add, Instr::Push(k), Instr::Sub],
                5 => vec![Instr::Push(1), Instr::Mul],
                6 => vec![Instr::Dup, Instr::Swap, Instr::Pop],
                // dead read: copy a local and discard it (no state change)
                _ => {
                    let l = rng.below(N_LOCALS) as u8;
                    vec![Instr::Load(l), Instr::Pop]
                }
            }
        }
        // 20%: a single instruction (typecheck usually rejects; cheap probe)
        50..=69 => vec![random_instr(rng)],
        // 30%: a short random range (ranges with net effect 0 can pass)
        _ => {
            let len = 2 + rng.below(3);
            (0..len).map(|_| random_instr(rng)).collect()
        }
    }
}

fn random_instr(rng: &mut Rng) -> Instr {
    match rng.below(10) {
        0 => Instr::Push(rng.magic_i32()),
        1 => Instr::Load(rng.below(N_LOCALS) as u8),
        2 => Instr::Store(rng.below(N_LOCALS) as u8),
        3 => Instr::Pop,
        4 => Instr::Dup,
        5 => Instr::Swap,
        6 => Instr::Add,
        7 => Instr::Sub,
        8 => Instr::Mul,
        _ => Instr::Xor,
    }
}

// ---------------------------------------------------------------------------
// Oracle
// ---------------------------------------------------------------------------

struct Oracle {
    vectors: Vec<[i32; N_LOCALS]>,
    /// `None` = original did not halt on this vector (excluded from demand).
    expected: Vec<Option<(i32, [i32; N_LOCALS])>>,
    budget: u64,
}

impl Oracle {
    fn new(original: &[Instr], n: usize, seed: u64, budget: u64) -> Oracle {
        let mut rng = Rng::new(seed);
        let mut vectors = Vec::with_capacity(n);
        let mut expected = Vec::with_capacity(n);
        for _ in 0..n {
            let v = rng.gen_vector();
            vectors.push(v);
            let (out, locals) = exec(original, &v, budget);
            expected.push(match out {
                Outcome::Halt(h) => Some((h, locals)),
                _ => None,
            });
        }
        Oracle {
            vectors,
            expected,
            budget,
        }
    }

    fn used(&self) -> usize {
        self.expected.iter().filter(|e| e.is_some()).count()
    }

    /// Must reproduce every original halt exactly: same result, same locals,
    /// within budget (candidate traps/step-limits are divergence).
    fn consistent(&self, candidate: &[Instr]) -> bool {
        for (v, exp) in self.vectors.iter().zip(&self.expected) {
            let Some(exp) = exp else { continue };
            let (out, locals) = exec(candidate, v, self.budget);
            match out {
                Outcome::Halt(h) => {
                    if h != exp.0 || locals != exp.1 {
                        return false;
                    }
                }
                _ => return false,
            }
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Fitness
// ---------------------------------------------------------------------------

fn score(mode: Mode, prog: &[Instr]) -> i64 {
    match mode {
        Mode::Size => -(prog.len() as i64),
        Mode::Obfuscate => {
            let mut s = 0i64;
            for w in prog.windows(2) {
                s -= match (w[0], w[1]) {
                    (Instr::Push(_), Instr::Pop) => 15,
                    (Instr::Load(_), Instr::Pop) => 10,
                    (Instr::Store(a), Instr::Load(b)) if a == b => 15,
                    (Instr::Store(a), Instr::Store(b)) if a == b => 20,
                    (Instr::Load(a), Instr::Store(b)) if a == b => 10,
                    (Instr::Push(0), Instr::Add | Instr::Sub) => 12,
                    (Instr::Push(1), Instr::Mul) => 12,
                    _ => 0,
                };
            }
            for i in prog {
                s += match i {
                    Instr::Dup | Instr::Swap | Instr::Xor => 8,
                    _ => 0,
                };
            }
            s - prog.len() as i64 // mild size tie-break, bounds runaway noise
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn check(flat: &[Instr]) -> Result<(), String> {
        Ir::split(flat).and_then(|ir| ir.typecheck())
    }

    fn optimize_src(src: &str, mode: Mode, seed: u64, iterations: u64) -> Report {
        let prog = parse(src).expect("parse");
        let mut cfg = Config::new(mode);
        cfg.seed = seed;
        cfg.iterations = iterations;
        optimize(&prog, &cfg).expect("optimize")
    }

    #[test]
    fn split_flatten_roundtrip_with_jumps() {
        let src = "\
    push 0
    store 0
loop:
    load 0
    push 0
    jlt done
    load 0
    push 1
    sub
    store 0
    jmp loop
    push 9
    pop
done:
    load 0
    ret
";
        let prog = parse(src).unwrap();
        let ir = Ir::split(&prog).unwrap();
        assert_eq!(ir.flatten(), prog, "split/flatten must be identity");
        ir.typecheck().unwrap();
    }

    #[test]
    fn split_maps_jump_to_end_to_sink() {
        let prog = parse("push 3\njmp end\nend:").unwrap();
        let ir = Ir::split(&prog).unwrap();
        assert_eq!(ir.flatten(), prog);
        ir.typecheck().unwrap();
    }

    #[test]
    fn typecheck_rejects_underflow() {
        let err = check(&parse("pop\nret\n").unwrap()).unwrap_err();
        assert!(err.contains("underflow"), "{err}");
    }

    #[test]
    fn typecheck_rejects_ret_with_empty_stack() {
        let err = check(&parse("ret\n").unwrap()).unwrap_err();
        assert!(err.contains("underflow"), "{err}");
    }

    #[test]
    fn typecheck_rejects_depth_conflict_at_merge() {
        let src = "\
    load 0
    push 5
    jlt yes
    push 100
    jmp join
yes:
    push 200
    push 7
join:
    ret
";
        let err = check(&parse(src).unwrap()).unwrap_err();
        assert!(err.contains("conflict"), "{err}");
    }

    #[test]
    fn typecheck_accepts_balanced_branches() {
        let src = "\
    load 0
    push 5
    jlt yes
    push 100
    jmp join
yes:
    push 200
join:
    load 1
    add
    ret
";
        check(&parse(src).unwrap()).unwrap();
    }

    #[test]
    fn typecheck_rejects_unbalanced_loop_backedge() {
        // loop body that grows the stack each iteration
        let src = "\
loop:
    load 0
    push 1
    sub
    store 0
    push 42
    load 0
    push 0
    jlt loop
    ret
";
        let err = check(&parse(src).unwrap()).unwrap_err();
        assert!(
            err.contains("conflict") || err.contains("underflow"),
            "{err}"
        );
    }

    #[test]
    fn typecheck_accepts_balanced_loop() {
        let src = "\
    push 5
    store 0
loop:
    load 0
    push 1
    sub
    store 0
    load 0
    push 0
    jlt loop
    load 0
    ret
";
        check(&parse(src).unwrap()).unwrap();
    }

    #[test]
    fn identity_units_are_semantically_neutral() {
        let units: Vec<Vec<Instr>> = vec![
            vec![Instr::Push(123), Instr::Pop],
            vec![Instr::Dup, Instr::Pop],
            vec![Instr::Swap, Instr::Swap],
            vec![
                Instr::Push(0x5a5a_5a5a),
                Instr::Xor,
                Instr::Push(0x5a5a_5a5a),
                Instr::Xor,
            ],
            vec![Instr::Push(999), Instr::Add, Instr::Push(999), Instr::Sub],
            vec![Instr::Push(1), Instr::Mul],
            vec![Instr::Dup, Instr::Swap, Instr::Pop],
            vec![Instr::Load(4), Instr::Pop],
            // (no Store unit: all 16 locals are observable, so *every* write
            // — even to a never-loaded slot — diverges by design)
        ];
        // base: compute something observable from locals
        let base = parse("load 0\nload 1\nxor\nstore 2\nload 2\nret").unwrap();
        let mut rng = Rng::new(0xbeef);
        for unit in units {
            // Insert the unit where stack depth is 2 (after `load 0; load 1`)
            // so depth-sensitive units like [swap, swap] are well-defined.
            let mut ir = Ir::split(&base).unwrap();
            let pos = 2;
            for (k, ins) in unit.iter().enumerate() {
                ir.blocks[0].instrs.insert(pos + k, *ins);
            }
            ir.typecheck().expect("unit must keep the program typable");
            let flat = ir.flatten();
            for _ in 0..16 {
                let v = rng.gen_vector();
                let want = exec(&base, &v, DEFAULT_BUDGET);
                let got = exec(&flat, &v, DEFAULT_BUDGET);
                assert_eq!(want, got, "unit diverged: {:?}", unit);
            }
        }
    }

    #[test]
    fn size_mode_removes_dead_code_and_is_verified() {
        let rep = optimize_src(
            "\
    push 5
    pop
    load 0
    load 1
    xor
    push 0x5a5a5a5a
    xor
    push 0x5a5a5a5a
    xor
    push 1
    mul
    store 2
    load 2
    ret
",
            Mode::Size,
            7,
            6_000,
        );
        assert!(rep.verified, "candidate must pass fresh verification");
        assert!(
            rep.candidate_len < rep.original_len,
            "size mode must shrink: {} -> {}",
            rep.original_len,
            rep.candidate_len
        );
        assert!(rep.rejected_divergent > 0, "oracle must have filtered");
    }

    #[test]
    fn obfuscate_mode_adds_structural_noise() {
        // NOTE: locals[0] holds the input — a fixture must never store into
        // it, or every vector livelocks (the oracle would reject all of them).
        let src = "\
    push 8
    store 1
loop:
    load 1
    push 0
    jlt done
    load 0
    load 1
    xor
    store 2
    load 1
    push 1
    sub
    store 1
    jmp loop
done:
    load 2
    ret
";
        let rep = optimize_src(src, Mode::Obfuscate, 11, 4_000);
        assert!(rep.verified, "candidate must pass fresh verification");
        assert!(
            rep.candidate_len > rep.original_len,
            "obfuscation should grow the program: {} -> {}",
            rep.original_len,
            rep.candidate_len
        );
        assert!(
            rep.candidate
                .iter()
                .any(|i| matches!(i, Instr::Dup | Instr::Swap | Instr::Xor)),
            "noise should include dup/swap/xor vocabulary"
        );
    }

    #[test]
    fn determinism_same_seed_same_report() {
        let src = "\
    load 0
    load 1
    add
    store 2
    load 2
    ret
";
        let a = optimize_src(src, Mode::Size, 99, 1_500);
        let b = optimize_src(src, Mode::Size, 99, 1_500);
        assert_eq!(a, b, "same seed must yield identical reports");
    }

    #[test]
    fn original_failing_typing_is_an_error() {
        let prog = parse("pop\nret").unwrap();
        let cfg = Config::new(Mode::Size);
        let err = optimize(&prog, &cfg).unwrap_err();
        assert!(matches!(err, OptError::UntypableOriginal(_)));
    }

    #[test]
    fn livelock_original_is_rejected() {
        let prog = parse("self:\n    jmp self").unwrap();
        let cfg = Config::new(Mode::Size);
        let err = optimize(&prog, &cfg).unwrap_err();
        assert!(matches!(err, OptError::NoValidVectors));
    }

    #[test]
    fn mutations_keep_terminators_pinned() {
        let prog = parse(
            "\
    load 0
    push 1
    jlt a
    jmp b
a:
    push 2
    pop
b:
    ret
",
        )
        .unwrap();
        let mut ir = Ir::split(&prog).unwrap();
        let term_positions: Vec<(usize, usize)> = ir
            .blocks
            .iter()
            .enumerate()
            .filter(|(_, b)| b.instrs.last().is_some_and(Instr::is_terminator))
            .map(|(i, b)| (i, b.instrs.len()))
            .collect();
        let mut rng = Rng::new(1234);
        for _ in 0..500 {
            ir.mutate(&mut rng);
            for &(bi, _) in &term_positions {
                if bi < ir.blocks.len() {
                    assert!(
                        ir.blocks[bi]
                            .instrs
                            .last()
                            .is_some_and(Instr::is_terminator),
                        "terminator must stay pinned as last of block {bi}"
                    );
                }
            }
            // every accepted flat program must remain executable without BadPc
            let flat = ir.flatten();
            let max_target = flat
                .iter()
                .filter_map(|i| match i {
                    Instr::Jeq(t) | Instr::Jlt(t) | Instr::Jmp(t) => Some(*t),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            assert!(max_target <= flat.len(), "jump target out of range");
            // keep the IR loadable for the next round
            if ir.typecheck().is_err() {
                ir = Ir::split(&prog).unwrap();
            }
        }
    }
}
