use std::io::Write;
use std::process::{Command, Stdio};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_json-schema-to-zod"))
}

fn fixture() -> String {
    format!("{}/tests/fixtures/all.json", env!("CARGO_MANIFEST_DIR"))
}

/// Run the CLI with args and an optional stdin, returning (stdout, stderr).
fn run(args: &[&str], stdin: Option<&str>) -> (String, String) {
    let mut cmd = bin();
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });

    let mut child = cmd.spawn().expect("spawn cli");
    if let Some(input) = stdin {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    let out = child.wait_with_output().expect("wait cli");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn help_lists_input() {
    let (stdout, _) = run(&["-h"], None);
    assert!(stdout.contains("--input"));
}

#[test]
fn input_file_no_stderr() {
    let (_, stderr) = run(&["-i", &fixture()], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn no_import_no_stderr() {
    let (_, stderr) = run(&["-i", &fixture(), "--noImport"], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn stdin_only_no_stderr() {
    let (_, stderr) = run(&[], Some("{\"type\": \"any\"}"));
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn output_file_no_stderr() {
    let dir = std::env::temp_dir().join("jstz_cli_out");
    let out = dir.join("output.js");
    let out_str = out.to_str().unwrap();
    let (_, stderr) = run(&["--output", out_str, "-i", &fixture()], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(out.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn output_with_depth_no_stderr() {
    let dir = std::env::temp_dir().join("jstz_cli_depth");
    let out = dir.join("output.js");
    let out_str = out.to_str().unwrap();
    let (_, stderr) = run(&["--output", out_str, "-i", &fixture(), "-d", "2"], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn missing_input_json_error() {
    let dir = std::env::temp_dir().join("jstz_cli_missing");
    let out = dir.join("output.js");
    let out_str = out.to_str().unwrap();
    let (_, stderr) = run(&["--output", out_str], Some(""));
    assert!(
        stderr.contains("Unexpected end of JSON input"),
        "stderr: {stderr}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn bad_depth_error() {
    let (_, stderr) = run(&["-i", &fixture(), "-d", "abc"], None);
    assert!(
        stderr.contains("Value of argument depth must be a valid number"),
        "stderr: {stderr}"
    );
}

#[test]
fn missing_depth_value_error() {
    let (_, stderr) = run(&["-i", &fixture(), "-d"], None);
    assert!(
        stderr.contains("Expected a value for argument depth"),
        "stderr: {stderr}"
    );
}

#[test]
fn depth_nan_is_rejected() {
    // JS `Number("nan")` is NaN, so the CLI rejects it.
    let (_, stderr) = run(&["-i", &fixture(), "-d", "nan"], None);
    assert!(
        stderr.contains("Value of argument depth must be a valid number"),
        "stderr: {stderr}"
    );
}

#[test]
fn depth_hex_is_accepted() {
    // JS `Number("0x10")` is 16, a valid depth.
    let (_, stderr) = run(&["-i", &fixture(), "-d", "0x10"], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn depth_infinity_is_accepted() {
    // JS `Number("Infinity")` is Infinity, which means unbounded depth.
    let (_, stderr) = run(&["-i", &fixture(), "-d", "Infinity"], None);
    assert!(stderr.is_empty(), "stderr: {stderr}");
}

#[test]
fn bad_module_error() {
    let (_, stderr) = run(&["-i", &fixture(), "-m", "notAModule"], None);
    assert!(
        stderr.contains("Value of argument module must be one of esm,cjs,none"),
        "stderr: {stderr}"
    );
}

// Added end-to-end checks.

#[test]
fn zod_version_three_record() {
    // An explicit object schema produces v3 record syntax with no key type.
    let (stdout, stderr) = run(&["--zodVersion", "3"], Some("{\"type\": \"object\"}"));
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.contains("z.record(z.any())"), "stdout: {stdout}");
}

#[test]
fn name_and_type_export() {
    let (stdout, stderr) = run(
        &["-n", "s", "-m", "esm", "-t", "S"],
        Some("{\"type\": \"string\"}"),
    );
    assert!(stderr.is_empty(), "stderr: {stderr}");
    assert!(stdout.contains("export const s ="), "stdout: {stdout}");
    assert!(stdout.contains("export type S ="), "stdout: {stdout}");
}
