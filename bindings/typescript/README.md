# TypeScript bridge

The TypeScript backend has two parts:

- `native/` is a Rust `cdylib` built with `napi-rs`; its build script generates `NAME-VERSION.rs` and `typescript.NAME-VERSION.rs` in Cargo `OUT_DIR` and exports the Node-API addon.
- `src/` is the typed TypeScript facade and loader.

```bash
cargo build --manifest-path bindings/typescript/native/Cargo.toml
npm ci
npm run build
```

The native module exposes ABI/Layer negotiation, `encodeJson(layer, typeName, input)` / `decodeJson(...)`, and `validateBinary(layer, typeName, input)`. `validateBinary` runs the generated checked borrowed decoder without materializing an owned object; the caller retains the original `Uint8Array`. Unknown types, unsupported Layers, malformed JSON, and invalid wire bytes become Node errors.
