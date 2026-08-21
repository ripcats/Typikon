# Python bridge

The Python backend is a PyO3 extension. Its build script reads `TYPIKON_SCHEMA` (or defaults to `../../examples/messenger.typ`), generates `NAME-VERSION.rs` and the Python bridge in Cargo `OUT_DIR`, and compiles them into one Python extension.

```bash
cargo build --manifest-path bindings/python/Cargo.toml
```

The generated module exposes direct PyO3 encode/decode, `validate_borrowed_<type>`, `borrowed_<type>`, and `borrowed_packet_<type>` functions for every schema item. `BorrowedPacket` owns the packet-backed `memoryview` and its type name; Python strings, lists, and dictionaries remain materialized because of Python object ownership semantics. JSON serialization is not used at the Python/Rust boundary.
