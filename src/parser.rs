//! Text bytecode: human-writable assembly ↔ `Vec<Instr>`.
//!
//! Format: one instruction or label per line, `#` comments, labels are
//! `name:` (harvested in pass 1), jump arguments are labels or absolute
//! instruction indices. Example:
//!
//! ```text
//!     push 5
//!     store 0
//! loop:
//!     load 0
//!     push 1
//!     sub
//!     store 0
//!     load 0
//!     jlt loop
//!     ret
//! ```

use std::collections::HashMap;
use std::fmt;

use crate::vm::{Instr, N_LOCALS};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub msg: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.msg)
    }
}

impl std::error::Error for ParseError {}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, ParseError> {
    Err(ParseError {
        line,
        msg: msg.into(),
    })
}

fn is_ident(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    cs.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Parse a numeric literal: decimal or `0x` hex, negatives allowed.
fn parse_i32(tok: &str) -> Option<i32> {
    if let Some(hex) = tok.strip_prefix("0x") {
        return i32::from_str_radix(hex, 16)
            .or_else(|_| u32::from_str_radix(hex, 16).map(|v| v as i32))
            .ok();
    }
    tok.parse::<i32>().ok()
}

fn need_one<'a>(toks: &'a [&'a str], line: usize, mnem: &str) -> Result<&'a str, ParseError> {
    match *toks {
        [one] => Ok(one),
        [] => err(line, format!("`{mnem}` takes exactly one operand")),
        _ => err(line, format!("`{mnem}` takes exactly one operand")),
    }
}

fn need_none(toks: &[&str], line: usize, mnem: &str) -> Result<(), ParseError> {
    if toks.is_empty() {
        Ok(())
    } else {
        err(
            line,
            format!("`{mnem}` takes no operands, got {}", toks.len()),
        )
    }
}

enum Pending {
    Plain(Instr),
    Jump {
        kind: &'static str, // "jeq" | "jlt" | "jmp"
        target: String,
    },
}

fn add_label(
    labels: &mut HashMap<String, usize>,
    name: &str,
    pc: usize,
    line: usize,
) -> Result<(), ParseError> {
    if labels.insert(name.to_string(), pc).is_some() {
        return err(line, format!("duplicate label `{name}`"));
    }
    Ok(())
}

/// Assemble asm text into a program (labels resolved, indices validated).
pub fn parse(text: &str) -> Result<Vec<Instr>, ParseError> {
    let mut labels: HashMap<String, usize> = HashMap::new();
    let mut pending: Vec<(usize, Pending)> = Vec::new();

    for (idx, raw) in text.lines().enumerate() {
        let lineno = idx + 1;
        let mut line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        // Optional leading label: `name:` alone or `name: instr...`
        if let Some((first, rest)) = line.split_once(char::is_whitespace) {
            if let Some(name) = first.strip_suffix(':')
                && is_ident(name)
            {
                add_label(&mut labels, name, pending.len(), lineno)?;
                line = rest.trim();
                if line.is_empty() {
                    continue;
                }
            }
        } else if let Some(name) = line.strip_suffix(':') {
            if !is_ident(name) {
                return err(lineno, format!("invalid label name `{name}`"));
            }
            add_label(&mut labels, name, pending.len(), lineno)?;
            continue;
        }

        let normalized = line.replace(',', " ");
        let mut toks: Vec<&str> = normalized.split_whitespace().collect();
        if toks.is_empty() {
            continue;
        }
        // Tolerate canonical `  N:` pc annotations (as emitted by `disassemble`).
        if let Some(num) = toks[0].strip_suffix(':')
            && !num.is_empty()
            && num.bytes().all(|b| b.is_ascii_digit())
        {
            let annotated: usize = num.parse().expect("digits");
            if annotated != pending.len() {
                return err(
                    lineno,
                    format!(
                        "pc annotation {annotated} does not match position {}",
                        pending.len()
                    ),
                );
            }
            toks.remove(0);
            if toks.is_empty() {
                continue;
            }
        }
        let mnem = toks.remove(0).to_ascii_lowercase();
        let p = match mnem.as_str() {
            "push" => {
                let a = need_one(&toks, lineno, "push")?;
                match parse_i32(a) {
                    Some(v) => Pending::Plain(Instr::Push(v)),
                    None => return err(lineno, format!("bad i32 literal `{a}`")),
                }
            }
            "pop" => {
                need_none(&toks, lineno, "pop")?;
                Pending::Plain(Instr::Pop)
            }
            "load" | "store" => {
                let a = need_one(&toks, lineno, &mnem)?;
                match a.parse::<u8>() {
                    Ok(l) if l < N_LOCALS as u8 => Pending::Plain(if mnem == "load" {
                        Instr::Load(l)
                    } else {
                        Instr::Store(l)
                    }),
                    Ok(l) => {
                        return err(
                            lineno,
                            format!("local index {l} out of range (0..={})", N_LOCALS - 1),
                        );
                    }
                    Err(_) => return err(lineno, format!("bad local index `{a}`")),
                }
            }
            "add" | "sub" | "mul" | "xor" | "dup" | "swap" | "ret" => {
                need_none(&toks, lineno, &mnem)?;
                Pending::Plain(match mnem.as_str() {
                    "add" => Instr::Add,
                    "sub" => Instr::Sub,
                    "mul" => Instr::Mul,
                    "xor" => Instr::Xor,
                    "dup" => Instr::Dup,
                    "swap" => Instr::Swap,
                    _ => Instr::Ret,
                })
            }
            "jeq" | "jlt" | "jmp" => {
                let target = need_one(&toks, lineno, &mnem)?.to_string();
                Pending::Jump {
                    kind: match mnem.as_str() {
                        "jeq" => "jeq",
                        "jlt" => "jlt",
                        _ => "jmp",
                    },
                    target,
                }
            }
            other => return err(lineno, format!("unknown mnemonic `{other}`")),
        };
        pending.push((lineno, p));
    }

    // Pass 2: resolve jump targets (label or absolute index), validate range.
    let len = pending.len();
    let mut out = Vec::with_capacity(len);
    for (lineno, p) in pending {
        out.push(match p {
            Pending::Plain(i) => i,
            Pending::Jump { kind, target } => {
                let resolved = if let Some(&pc) = labels.get(&target) {
                    pc
                } else if let Ok(pc) = target.parse::<usize>() {
                    if pc > len {
                        return err(
                            lineno,
                            format!(
                                "jump target {pc} out of range (program is {len} instructions)"
                            ),
                        );
                    }
                    pc
                } else {
                    return err(lineno, format!("undefined label `{target}`"));
                };
                match kind {
                    "jeq" => Instr::Jeq(resolved),
                    "jlt" => Instr::Jlt(resolved),
                    _ => Instr::Jmp(resolved),
                }
            }
        });
    }
    Ok(out)
}

/// Render a program back to asm text (canonical, re-parsable).
pub fn disassemble(prog: &[Instr]) -> String {
    let mut s = String::new();
    for (pc, i) in prog.iter().enumerate() {
        s.push_str(&format_instruction(pc, i));
        s.push('\n');
    }
    s
}

/// Canonical single-line rendering of one instruction at `pc`.
pub fn format_instruction(pc: usize, i: &Instr) -> String {
    match i {
        Instr::Push(v) => format!("{pc:>4}: push {v}"),
        Instr::Pop => format!("{pc:>4}: pop"),
        Instr::Load(l) => format!("{pc:>4}: load {l}"),
        Instr::Store(l) => format!("{pc:>4}: store {l}"),
        Instr::Add => format!("{pc:>4}: add"),
        Instr::Sub => format!("{pc:>4}: sub"),
        Instr::Mul => format!("{pc:>4}: mul"),
        Instr::Xor => format!("{pc:>4}: xor"),
        Instr::Dup => format!("{pc:>4}: dup"),
        Instr::Swap => format!("{pc:>4}: swap"),
        Instr::Jeq(t) => format!("{pc:>4}: jeq {t}"),
        Instr::Jlt(t) => format!("{pc:>4}: jlt {t}"),
        Instr::Jmp(t) => format!("{pc:>4}: jmp {t}"),
        Instr::Ret => format!("{pc:>4}: ret"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::{DEFAULT_BUDGET, Outcome, Vm};

    #[test]
    fn roundtrip_disassemble_reparse() {
        let src = "\
    push 0x2a
    store 15
top:
    load 15
    push 1
    sub
    dup
    store 15
    push 0
    jlt top
    swap
    pop
    xor
    mul
    jeq top
    jmp top
    ret
";
        let prog = parse(src).expect("parse");
        let text = disassemble(&prog);
        let prog2 = parse(&text).expect("reparse");
        assert_eq!(prog, prog2);
        assert_eq!(text, disassemble(&prog2));
    }

    #[test]
    fn labels_comments_and_inline_label() {
        let src = "\
# a comment
start: push 1   # trailing comment
    jlt start
    ret
";
        let prog = parse(src).unwrap();
        assert_eq!(prog, vec![Instr::Push(1), Instr::Jlt(0), Instr::Ret]);
    }

    #[test]
    fn label_at_end_targets_halt() {
        let prog = parse("push 3\njmp end\nend:").unwrap();
        assert_eq!(prog, vec![Instr::Push(3), Instr::Jmp(2)]);
        let out = Vm::new().run(&prog, DEFAULT_BUDGET);
        assert_eq!(out, Outcome::Halt(3));
    }

    #[test]
    fn absolute_numeric_jump_target() {
        let prog = parse("push 1\njmp 2\n").unwrap();
        assert_eq!(prog[1], Instr::Jmp(2));
    }

    #[test]
    fn errors_carry_line_numbers() {
        let cases = [
            ("push 1\nbogus", "unknown mnemonic"),
            ("push 1\nload 16", "out of range"),
            ("jmp nowhere", "undefined label"),
            ("push 99999999999", "bad i32 literal"),
            ("push 1\npush 2 extra", "exactly one"),
            ("dup 3", "no operands"),
            ("a: push 1\na: push 2", "duplicate label"),
            ("jmp 99", "out of range"),
        ];
        for (src, want) in cases {
            let e = parse(src).expect_err(src);
            assert!(
                e.msg.contains(want),
                "src={src:?}: got {:?}, want substring {want:?}",
                e.msg
            );
        }
        assert_eq!(parse("push 1\nbogus").unwrap_err().line, 2);
    }

    #[test]
    fn negative_and_extreme_locals_parse() {
        let prog = parse("push -5\nstore 0\nload 15\n").unwrap();
        assert_eq!(prog[0], Instr::Push(-5));
        assert_eq!(prog[1], Instr::Store(0));
        assert_eq!(prog[2], Instr::Load(15));
    }
}
