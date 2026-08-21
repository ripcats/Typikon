# Python bridge

The Python backend is a PyO3 extension. Its build script reads `TYPIKON_SCHEMA` (or defaults to `../../examples/messenger.typ`), generates `NAME-VERSION.rs` and the Python bridge in Cargo `OUT_DIR`, and compiles them into one Python extension.

```bash
cargo build --manifest-path bindings/python/Cargo.toml
```

The generated module exposes direct PyO3 encode/decode functions for every schema item. Python dictionaries, lists, and scalar values are converted directly to generated Rust values with `pythonize`; JSON serialization is not used at the Python/Rust boundary. Wire encoding remains implemented by the generated native Rust backend.
