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
- Added a reproducible Typikon/FlatBuffers comparison harness with wire size, encode/decode, iteration, and allocation metrics.
- Expanded the comparison harness with 64 KiB and 1 MiB binary-payload cases.
- Added backward-compatible Layer transition checks for existing constructors, enum variants, and flags.
- Generated Rust codecs can now decode with caller-supplied packet, collection, and byte-field limits.
- Added `tests/long_validation.sh` for repeatable multi-hour or multi-day validation runs.
- FlatBuffers comparison now reports verified and unchecked view decoding separately, making validation overhead explicit.
- Borrowed collection boundary scans now use structural skip paths for length-delimited values and generated nested views.
- Documented the crate-root zero-copy API and the separate ownership contract still required for language-binding view handles.
- Reusable encoder buffers, exact generated size hints, borrowed string/byte decoding, and binary TypeScript bridge entry points.
- Reproducible `cargo bench --bench wire` benchmark for message and 64 KiB payload paths.
- Python, TypeScript, and Go golden wire round-trip checks.
- Expanded malformed-input, collection preflight, canonical VarInt, borrowed-storage, and fuzz coverage.
- Added runtime coverage for freshly generated Go borrowed views, including nested values, enum payloads, aliasing, and truncated wire.
- Added an owner-preserving TypeScript native `borrowBinary` entry point and Python `BorrowedPacket` wrappers for validated packet-backed access.
- Added generated Go `*LazyView` and TypeScript `decode*LazyView` collection APIs that rescan packet-backed ranges instead of allocating element metadata.
- Updated the checked-in Go Layer 10 artifact with lazy views and Map ordering/duplicate-key validation in borrowed and lazy paths.
- Added Map ordering and duplicate-key rejection checks to generated Go and TypeScript borrowed/lazy views.
- Propagated Go lazy views through nested named structs, so nested collections remain packet-backed instead of falling back to materialized views.
- Propagated TypeScript lazy views through nested named structs with owner-aware decoder callbacks.

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
