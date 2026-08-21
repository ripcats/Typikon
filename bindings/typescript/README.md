# TypeScript bridge

The TypeScript backend has two parts:

- `native/` is a Rust `cdylib` built with `napi-rs`; its build script generates `NAME-VERSION.rs` and `typescript.NAME-VERSION.rs` in Cargo `OUT_DIR` and exports the Node-API addon.
- `src/` is the typed TypeScript facade and loader.

```bash
cargo build --manifest-path bindings/typescript/native/Cargo.toml
npm ci
npm run build
```

The generated TypeScript module exposes typed `encode<Name>(value)` / `decode<Name>(wire)` functions, `decode<Name>View(wire)`, owner-preserving `borrow<Name>View(wire)`, and `decode<Name>LazyView(wire)` packet-backed lazy collections. The native module exposes `encodeBinary`, `decodeBinary`, `validateBinary`, and owner-preserving `borrowBinary`; the facade combines it with a typed decoder through `borrowTyped`. JSON is not used by either path.
