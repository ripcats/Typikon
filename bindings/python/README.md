# Python bridge

The Python backend is a PyO3 extension. Its build script reads `TYPIKON_SCHEMA` (or defaults to `../../examples/messenger.typ`), generates `NAME-VERSION.rs` and the Python bridge in Cargo `OUT_DIR`, and compiles them into one Python extension.

```bash
python -m pip install -e bindings/python
# or, for a local development install:
cd bindings/python && maturin develop
```

The package facade imports the PyO3 extension and exposes direct encode/decode, `validate_borrowed_<type>`, and `borrowed_packet(type_name, wire)`. `BorrowedPacket` owns the packet-backed `memoryview` and its type name; Python strings, lists, and dictionaries remain materialized because of Python object ownership semantics, so no unsafe typed borrowed field API is promised. Set `TYPIKON_SCHEMA` before installation to build a different schema artifact. JSON serialization is not used at the Python/Rust boundary.
