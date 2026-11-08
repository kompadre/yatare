//! End-to-end demo: naive jqLite on the harness-side VDOM.

use std::process::Command;

#[test]
fn jqlite_vdom_demo_end_to_end() {
    let out = Command::new("node")
        .arg("demo/harness.js")
        .arg("--yatare")
        .arg(env!("CARGO_BIN_EXE_yatare"))
        .output()
        .expect("node must be on PATH to run the demo harness");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "harness failed\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("ALL GREEN"), "{stdout}");
}
