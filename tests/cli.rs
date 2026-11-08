//! End-to-end CLI tests (samples are the fixtures).

use std::process::Command;

fn yatare() -> Command {
    Command::new(env!("CARGO_BIN_EXE_yatare"))
}

fn stdout_of(out: &std::process::Output) -> String {
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn run_sum_loop_sample() {
    let out = yatare()
        .args(["run", "samples/sum_loop.asm"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = stdout_of(&out);
    assert!(stdout.contains("result: 0"), "{stdout}");
    // n = 0, acc = 0, i = 1 after the zero-trip loop
    assert!(stdout.contains("[2]=1"), "{stdout}");
}

#[test]
fn run_rejects_broken_file() {
    let out = yatare()
        .args(["run", "samples/does_not_exist.asm"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}

#[test]
fn disasm_dead_code_sample() {
    let out = yatare()
        .args(["disasm", "samples/dead_code.asm"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = stdout_of(&out);
    assert!(stdout.contains("xor"), "{stdout}");
    assert!(stdout.contains("ret"), "{stdout}");
}

#[test]
fn optimize_size_mode_shrinks_and_verifies() {
    let out = yatare()
        .args([
            "optimize",
            "samples/dead_code.asm",
            "--mode",
            "size",
            "--seed",
            "7",
            "--iters",
            "6000",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = stdout_of(&out);
    assert!(stdout.contains("verification: PASS"), "{stdout}");
    assert!(stdout.contains("mode: Size"), "{stdout}");
}

#[test]
fn optimize_obfuscate_mode_verifies() {
    let out = yatare()
        .args([
            "optimize",
            "samples/sum_loop.asm",
            "--mode",
            "obfuscate",
            "--seed",
            "3",
            "--iters",
            "4000",
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{out:?}");
    let stdout = stdout_of(&out);
    assert!(stdout.contains("verification: PASS"), "{stdout}");
    assert!(stdout.contains("mode: Obfuscate"), "{stdout}");
}
