# Changelog

All notable changes to Typikon since its initial public beta are documented here.

## Unreleased — beta

### Added

- Public `BorrowedWireCodec<'a>` and `decode_borrowed_value` APIs.
- Generated `TypeRef<'a>` Rust views for zero-copy decoding of direct `String` and `Vec<u8>` fields.
- Borrowed views now propagate through direct named-struct fields, so nested `UserRef<'a>` values keep pointing into the original packet.
- Lazy `BorrowedVec` and `BorrowedMap` views now cover repeated fields, nested structures, and map entries without materializing owned collections; enum payloads also have generated borrowed views.
- Added transport-neutral `Encoder::write_vectored` for sending framing segments around a packet without an intermediate concatenation.
- The checked-in benchmark now includes a collection-heavy messenger path with owned decode, borrowed decode, and lazy iteration measurements.
- Reusable encoder buffers, exact generated size hints, borrowed string/byte decoding, and binary TypeScript bridge entry points.
- Reproducible `cargo bench --bench wire` benchmark for message and 64 KiB payload paths.
- Python, TypeScript, and Go golden wire round-trip checks.
- Expanded malformed-input, collection preflight, canonical VarInt, borrowed-storage, and fuzz coverage.

### Changed

- Generated codecs write Constructor IDs and fields directly into one preallocated buffer.
- Constructor IDs are emitted as ready-to-use `[u8; 8]` constants instead of being parsed from hexadecimal strings per packet.
- Generated `Vec<u8>` fields use bulk length-delimited copies instead of per-byte codec dispatch.
- Fixed-width collection size calculation is O(1), and decode validates required bytes before allocation.
- VarInt encoding is batched through a stack buffer; decoding now requires canonical representations.
- README documentation is mirrored in Russian and English and includes reproducible performance/test commands.

### Fixed

- Failed length-delimited writes no longer leave a partial length prefix in an encoder.
- Collection counts use checked `u64` to `usize` conversion for 32-bit correctness.
- Truncated fixed-width collections are rejected before allocation and iteration.
- The stale cross-language script and Go facade were updated to the current messenger Layer 10 schema.
- TypeScript package typechecking is available through `npm test`.

### Current zero-copy scope

- Zero-copy: direct Rust `String` → `&str`, `Vec<u8>` → `&[u8]`, nested named structs, lazy repeated/map views, and enum payloads.
- Still owned: language-bridge values that materialize JSON/application objects.
- Transport integration remains adapter-owned: TCP+TLS can use vectored writes, while QUIC/WebSocket/WebTransport keep their own framing and message boundaries.

## 0.2.0 — initial public beta

- Introduced the `.typ` schema parser, semantic validation, BLAKE3 Constructor IDs, Guard bits, Layer negotiation, Rust wire generation, public schema artifacts, CLI compilation, and native adapters for Python, Go, and TypeScript.
