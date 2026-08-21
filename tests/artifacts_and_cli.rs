use std::fs;
use std::path::PathBuf;
use std::process::Command;
use typikon::{compile_schema, parse_schema};

#[test]
fn public_demo_is_a_parseable_reproducible_artifact() {
    let source = include_str!("../examples/messenger.typ");
    let public = include_str!("../examples/messenger-10.public.typ");
    let artifacts = compile_schema(source, "messenger.typ").unwrap();
    assert_eq!(artifacts.public_schema, public);
    let parsed = parse_schema(public).unwrap();
    assert_eq!(parsed.version, 10);
    assert_eq!(parsed.items.len(), 6);
}

#[test]
fn cli_check_and_compile_work_end_to_end() {
    let binary = PathBuf::from(std::env::var("CARGO_BIN_EXE_typikon").unwrap());
    let schema = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/layers/layer-8.typ");
    let check = Command::new(&binary)
        .arg("check")
        .arg(&schema)
        .output()
        .unwrap();
    assert!(check.status.success());
    assert!(String::from_utf8_lossy(&check.stdout).contains("valid Layer 8"));

    let output_dir = std::env::temp_dir().join(format!("typikon-cli-{}", std::process::id()));
    let _ = fs::remove_dir_all(&output_dir);
    let compile = Command::new(&binary)
        .arg("compile")
        .arg(&schema)
        .arg("--out-dir")
        .arg(&output_dir)
        .arg("--target")
        .arg("python,golang,typescript")
        .output()
        .unwrap();
    assert!(compile.status.success());
    assert!(output_dir.join("layer-8.rs").is_file());
    assert!(output_dir.join("layer-8.public.typ").is_file());
    assert!(output_dir.join("python.layer-8.rs").is_file());
    assert!(output_dir.join("golang.layer-8.rs").is_file());
    assert!(output_dir.join("typescript.layer-8.rs").is_file());
    assert!(output_dir.join("layer-8.h").is_file());
    assert!(output_dir.join("layer-8.go").is_file());
    assert!(output_dir.join("layer_8.py").is_file());
    assert!(output_dir.join("layer-8.ts").is_file());
    fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn cli_defaults_to_native_rust_only_and_selects_one_target() {
    let binary = PathBuf::from(std::env::var("CARGO_BIN_EXE_typikon").unwrap());
    let schema = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/layers/layer-8.typ");
    let root = std::env::temp_dir().join(format!("typikon-targets-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let rust_dir = root.join("rust");
    let python_dir = root.join("python");
    fs::create_dir_all(&rust_dir).unwrap();
    fs::create_dir_all(&python_dir).unwrap();

    let rust = Command::new(&binary)
        .args(["compile", schema.to_str().unwrap(), "--out-dir"])
        .arg(&rust_dir)
        .output()
        .unwrap();
    assert!(rust.status.success());
    assert!(rust_dir.join("layer-8.rs").is_file());
    assert!(rust_dir.join("layer-8.public.typ").is_file());
    assert!(!rust_dir.join("layer_8.py").exists());
    assert!(!rust_dir.join("python.layer-8.rs").exists());

    let python = Command::new(&binary)
        .args(["compile", schema.to_str().unwrap(), "--out-dir"])
        .arg(&python_dir)
        .args(["--target", "python,typescript"])
        .output()
        .unwrap();
    assert!(python.status.success());
    assert!(python_dir.join("layer-8.rs").is_file());
    assert!(python_dir.join("layer_8.py").is_file());
    assert!(python_dir.join("python.layer-8.rs").is_file());
    assert!(python_dir.join("typescript.layer-8.rs").is_file());
    assert!(python_dir.join("layer-8.ts").is_file());
    assert!(!python_dir.join("layer-8.go").exists());
    assert!(!python_dir.join("layer-8.h").exists());

    fs::remove_dir_all(root).unwrap();
}
