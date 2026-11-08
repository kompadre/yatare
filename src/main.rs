//! yatare CLI entry point.

mod parser;
mod vm;

use std::process::exit;

use vm::{DEFAULT_BUDGET, N_LOCALS, Outcome};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("run") => cmd_run(&args[2..]),
        Some("disasm") => cmd_disasm(&args[2..]),
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
                let Some(v) = args.get(i + 1) else {
                    eprintln!("yatare run: --budget needs a value");
                    exit(2);
                };
                match v.parse::<u64>() {
                    Ok(n) => budget = n,
                    Err(_) => {
                        eprintln!("yatare run: bad --budget value `{v}`");
                        exit(2);
                    }
                }
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
