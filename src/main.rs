//! yatare CLI entry point.

mod optimizer;
mod parser;
mod vm;

use std::process::exit;

use optimizer::{Config, Mode, Report, optimize};
use vm::{DEFAULT_BUDGET, N_LOCALS, Outcome};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => cmd_run(&args[2..]),
        Some("disasm") => cmd_disasm(&args[2..]),
        Some("optimize") => cmd_optimize(&args[2..]),
        Some("help") | Some("--help") | Some("-h") | None => print_help(),
        Some(other) => {
            eprintln!("yatare: unknown command `{other}`\n");
            print_help();
            exit(2);
        }
    }
}

fn print_help() {
    println!(
        "yatare — stack-based VM bytecode superoptimizer / obfuscator

USAGE:
    yatare run <file.asm> [--budget N]
        Interpret a bytecode assembly file and print the outcome.

    yatare disasm <file.asm>
        Parse and print the canonical disassembly.

    yatare optimize <file.asm> [--mode size|obfuscate] [--seed N] [--iters N]
                              [--vectors N] [--verify N]
        Search for an oracle-equivalent program and verify it on fresh test
        vectors. Exit code 3 when verification fails.

    yatare help
        Show this help."
    );
}

fn read_program(path: &str) -> Vec<vm::Instr> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("yatare: cannot read {path}: {e}");
            exit(1);
        }
    };
    match parser::parse(&text) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("yatare: {path}: {e}");
            exit(1);
        }
    }
}

fn cmd_run(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("yatare run: missing file argument");
        exit(2);
    };
    let mut budget = DEFAULT_BUDGET;
    let mut i = 1;
    while i < args.len() {
        match args[i].as_str() {
            "--budget" => {
                budget = flag_u64(args, i, "--budget");
                i += 2;
            }
            other => {
                eprintln!("yatare run: unexpected argument `{other}`");
                exit(2);
            }
        }
    }

    let prog = read_program(path);
    let (out, locals) = vm::exec(&prog, &[0; N_LOCALS], budget);
    match out {
        Outcome::Halt(v) => println!("result: {v}"),
        Outcome::Trap(t) => {
            println!("outcome: trap {t:?}");
            print_locals(&locals);
            exit(1);
        }
        Outcome::StepLimit => {
            println!("outcome: step limit exceeded ({budget} steps)");
            print_locals(&locals);
            exit(1);
        }
    }
    print_locals(&locals);
}

fn cmd_disasm(args: &[String]) {
    let Some(path) = args.first() else {
        eprintln!("yatare disasm: missing file argument");
        exit(2);
    };
    let prog = read_program(path);
    print!("{}", parser::disassemble(&prog));
}

fn cmd_optimize(args: &[String]) {
    let mut path: Option<&str> = None;
    let mut mode = Mode::Size;
    let mut seed = 42u64;
    let mut iterations = 6_000u64;
    let mut search_vectors = 64usize;
    let mut verify_vectors = 512usize;

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "--mode" => {
                mode = match args.get(i + 1).map(String::as_str) {
                    Some("size") => Mode::Size,
                    Some("obfuscate") => Mode::Obfuscate,
                    Some(other) => {
                        eprintln!("yatare optimize: bad --mode `{other}` (size|obfuscate)");
                        exit(2);
                    }
                    None => missing_value("--mode"),
                };
                i += 2;
            }
            "--seed" => {
                seed = flag_u64(args, i, "--seed");
                i += 2;
            }
            "--iters" => {
                iterations = flag_u64(args, i, "--iters");
                i += 2;
            }
            "--vectors" => {
                search_vectors = flag_u64(args, i, "--vectors") as usize;
                i += 2;
            }
            "--verify" => {
                verify_vectors = flag_u64(args, i, "--verify") as usize;
                i += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("yatare optimize: unknown flag `{other}`");
                exit(2);
            }
            other => {
                if path.is_some() {
                    eprintln!("yatare optimize: unexpected argument `{other}`");
                    exit(2);
                }
                path = Some(other);
                i += 1;
            }
        }
    }
    let Some(path) = path else {
        eprintln!("yatare optimize: missing file argument");
        exit(2);
    };

    let prog = read_program(path);
    let cfg = Config {
        mode,
        seed,
        iterations,
        search_vectors,
        verify_vectors,
        ..Config::new(mode)
    };
    match optimize(&prog, &cfg) {
        Ok(report) => {
            print_report(&report);
            if !report.verified {
                exit(3);
            }
        }
        Err(e) => {
            eprintln!("yatare: optimize failed: {e}");
            exit(1);
        }
    }
}

fn missing_value(flag: &str) -> ! {
    eprintln!("yatare optimize: {flag} needs a value");
    exit(2)
}

fn flag_u64(args: &[String], i: usize, flag: &str) -> u64 {
    let Some(v) = args.get(i + 1) else {
        eprintln!("yatare: {flag} needs a value");
        exit(2);
    };
    match v.parse::<u64>() {
        Ok(n) => n,
        Err(_) => {
            eprintln!("yatare: {flag}: bad value `{v}`");
            exit(2)
        }
    }
}

fn print_report(r: &Report) {
    println!("mode: {:?}  seed: {}", r.mode, r.seed);
    println!("--- before ({} instructions) ---", r.original_len);
    print!("{}", parser::disassemble(&r.original));
    println!("--- after ({} instructions) ---", r.candidate_len);
    print!("{}", parser::disassemble(&r.candidate));
    if r.original_len > 0 {
        let delta =
            100.0 * (r.candidate_len as f64 - r.original_len as f64) / r.original_len as f64;
        println!(
            "length: {} -> {} ({delta:+.1}%)",
            r.original_len, r.candidate_len
        );
    } else {
        println!("length: {} -> {}", r.original_len, r.candidate_len);
    }
    println!(
        "search: {} iterations | proposed {} | untypable {} | divergent {} | accepted {}",
        r.iterations, r.proposed, r.rejected_untypable, r.rejected_divergent, r.accepted
    );
    println!(
        "verification: {} ({} fresh vectors)",
        if r.verified { "PASS" } else { "FAIL" },
        r.verify_vectors_used
    );
}

fn print_locals(locals: &[i32; N_LOCALS]) {
    let nonzero: Vec<String> = locals
        .iter()
        .enumerate()
        .filter_map(|(i, &v)| (v != 0).then_some(format!("[{i}]={v}")))
        .collect();
    if nonzero.is_empty() {
        println!("locals: all zero");
    } else {
        println!("locals: {}", nonzero.join(" "));
    }
}
