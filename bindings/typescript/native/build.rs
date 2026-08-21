use std::{env, fs, path::PathBuf};

fn main() {
    let schema_path = env::var_os("TYPIKON_SCHEMA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("../../../examples/messenger.typ"));
    println!("cargo:rerun-if-changed={}", schema_path.display());
    let source = fs::read_to_string(&schema_path).expect("read TYPIKON_SCHEMA");
    let schema_name = schema_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("schema.typ");
    let artifacts = typikon::compile_schema(&source, schema_name).expect("compile TYPIKON_SCHEMA");
    let schema = typikon::parse_schema(&source).expect("parse TYPIKON_SCHEMA");
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    fs::write(out_dir.join(&artifacts.rust_file_name), &artifacts.rust_source)
        .expect("write native generated Rust");
    fs::write(
        out_dir.join(&artifacts.public_schema_file_name),
        &artifacts.public_schema,
    )
    .expect("write public schema");
    fs::write(
        out_dir.join("typescript-bridge.rs"),
        typikon::generate_bridge(
            &schema,
            &artifacts.rust_file_name,
            typikon::BridgeKind::TypeScript,
        ),
    )
    .expect("write typescript-bridge.rs");
}
