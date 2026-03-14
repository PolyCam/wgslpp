//! Miniray validation test suite.
//!
//! Adapts the miniray validation test data (~65 WGSL files with annotation-based
//! expectations) to verify naga's validation behavior.
//!
//! Annotation format:
//!   // @test: <test-name>
//!   // @expect-valid
//!   // @expect-error <CODE> ["message pattern"]
//!   // @spec-ref: <reference>
//!
//! Source: https://github.com/HugoDaniel/miniray (CC0)

use std::path::{Path, PathBuf};

use wgslpp_core::validate::validate;

/// Parsed expectations from a miniray test file.
#[allow(dead_code)]
struct TestCase {
    name: String,
    path: PathBuf,
    source: String,
    expect_valid: bool,
    expected_errors: Vec<ExpectedDiagnostic>,
}

#[allow(dead_code)]
struct ExpectedDiagnostic {
    code: String,
    pattern: Option<String>,
}

fn external_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("external")
        .join("miniray")
        .join("validation")
}

fn parse_test_file(path: &Path) -> TestCase {
    let source = std::fs::read_to_string(path).unwrap();
    let mut name = path
        .file_stem()
        .unwrap()
        .to_string_lossy()
        .to_string();
    let mut expect_valid = true; // default if no annotation
    let mut expected_errors = Vec::new();
    let mut has_explicit_expectation = false;

    for line in source.lines() {
        let trimmed = line.trim();

        // @test: <name>
        if let Some(rest) = trimmed.strip_prefix("// @test:") {
            name = rest.trim().to_string();
        }

        // @expect-valid
        if trimmed.contains("@expect-valid") {
            expect_valid = true;
            has_explicit_expectation = true;
        }

        // @expect-error CODE ["message"]
        if let Some(rest) = trimmed.strip_prefix("// @expect-error") {
            has_explicit_expectation = true;
            expect_valid = false;
            let rest = rest.trim();
            let mut parts = rest.splitn(2, '"');
            let code = parts
                .next()
                .unwrap_or("")
                .trim()
                .to_string();
            let pattern = parts
                .next()
                .map(|s| s.trim_end_matches('"').to_string());
            expected_errors.push(ExpectedDiagnostic { code, pattern });
        }
    }

    if !has_explicit_expectation {
        // Default: assume valid
        expect_valid = true;
    }

    TestCase {
        name,
        path: path.to_path_buf(),
        source,
        expect_valid,
        expected_errors,
    }
}

fn run_test_case(tc: &TestCase) {
    let result = validate(&tc.source, None);

    if tc.expect_valid {
        // Should validate without errors
        if !result.diagnostics.is_empty() {
            let msgs: Vec<_> = result
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .collect();
            // Some valid WGSL that miniray's parser accepts might not pass naga
            // validation (different strictness levels). We mark these as known
            // divergences rather than failures.
            panic!(
                "[{}] expected valid, but got {} error(s):\n{}",
                tc.name,
                msgs.len(),
                msgs.join("\n")
            );
        }
    } else {
        // Should produce errors
        if result.diagnostics.is_empty() && result.module.is_some() && result.module_info.is_some()
        {
            panic!(
                "[{}] expected error(s) {:?}, but validation succeeded",
                tc.name,
                tc.expected_errors
                    .iter()
                    .map(|e| &e.code)
                    .collect::<Vec<_>>()
            );
        }
        // Note: We don't check the specific error codes because miniray uses its
        // own error code system (E0100, E0200, etc.) which differs from naga's
        // error types. We only verify that naga also rejects these invalid shaders.
    }
}

fn collect_wgsl_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() {
            files.extend(collect_wgsl_files(&path));
        } else if path.extension().map_or(false, |ext| ext == "wgsl") {
            files.push(path);
        }
    }
    files.sort();
    files
}

// ---- Valid shader tests ----

#[test]
fn miniray_validation_builtins() {
    let dir = external_dir().join("builtins");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_declarations() {
    let dir = external_dir().join("declarations");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_expressions() {
    let dir = external_dir().join("expressions");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_types() {
    let dir = external_dir().join("types");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_uniformity() {
    let dir = external_dir().join("uniformity");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

// ---- Error tests (expect naga to also reject) ----

#[test]
fn miniray_validation_errors_calls() {
    let dir = external_dir().join("errors").join("calls");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_errors_control_flow() {
    let dir = external_dir().join("errors").join("control_flow");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_errors_declarations() {
    let dir = external_dir().join("errors").join("declarations");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_errors_operations() {
    let dir = external_dir().join("errors").join("operations");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_errors_symbols() {
    let dir = external_dir().join("errors").join("symbols");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

#[test]
fn miniray_validation_errors_types() {
    let dir = external_dir().join("errors").join("types");
    for path in collect_wgsl_files(&dir) {
        let tc = parse_test_file(&path);
        run_test_case(&tc);
    }
}

// ---- Aggregate test ----

#[test]
fn miniray_validation_all() {
    let dir = external_dir();
    let files = collect_wgsl_files(&dir);
    assert!(
        !files.is_empty(),
        "no validation test files found in {}",
        dir.display()
    );

    let mut passed = 0;
    let mut failed = 0;
    let mut failures = Vec::new();

    for path in &files {
        let tc = parse_test_file(path);
        let result = std::panic::catch_unwind(|| run_test_case(&tc));
        match result {
            Ok(()) => passed += 1,
            Err(e) => {
                failed += 1;
                let msg = if let Some(s) = e.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = e.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "unknown panic".to_string()
                };
                failures.push((tc.name.clone(), msg));
            }
        }
    }

    eprintln!(
        "\n=== Miniray Validation Summary ===\n  Total: {}\n  Passed: {}\n  Failed: {}",
        files.len(),
        passed,
        failed
    );

    for (name, msg) in &failures {
        eprintln!("  FAIL: {} — {}", name, msg);
    }

    // We expect all tests to pass (naga should agree with miniray on valid/invalid)
    assert!(
        failures.is_empty(),
        "{} test(s) failed out of {}",
        failed,
        files.len()
    );
}
