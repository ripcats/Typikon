# Typikon Protocol

![Typikon Protocol](assets/cover.png)

[![Version](https://img.shields.io/badge/Version-Beta-5865F2?style=for-the-badge&logo=github&logoColor=white)](#what-is-actually-tested)
[![Tests](https://img.shields.io/badge/Tests-64%20Passing-3FB950?style=for-the-badge&logo=githubactions&logoColor=white)](#what-is-actually-tested)
[![Русский](https://img.shields.io/badge/%D0%A0%D1%83%D1%81%D1%81%D0%BA%D0%B8%D0%B9-2D333B?style=for-the-badge&logo=libretranslate&logoColor=white)](README.md)
[![Evgeny Gerber](https://img.shields.io/badge/Evgeny%20Gerber-2AABEE?style=for-the-badge&logo=telegram&logoColor=white)](https://ripcats.t.me)

**Typikon is a schema language and compiler for a typed binary wire protocol.**

Define the contract in a human-readable `.typ` schema — Typikon validates its semantics and produces a schema-specific Rust wire core, a public schema with computed **Constructor ID (C-ID)** values, and official cross-platform adapters for Python, Go, and TypeScript.

The Rust core supports zero-copy borrowed views for strings, bytes, and lazy collections; Go and TypeScript generate packet-backed lazy collection APIs and owner-preserving typed packet wrappers, while Python keeps an owner-backed packet `memoryview` within its runtime model.

> **Beta release.** The project builds, runs tests, and generates working artifacts, but the protocol format and C-ID rules may still change.

## Why Typikon

Instead of maintaining a binary format, serializers, and several language implementations by hand, you describe the contract once:

~~~text
schema.typ
    │
    ├── parser + semantic validation
    ├── canonical form + BLAKE3 C-ID
    ├── generated Rust wire core
    └── generated Python / Go / TypeScript adapters
~~~

The wire format remains binary. JSON is not part of Typikon's public wire or typed Go/TypeScript codec APIs; Python crosses the PyO3 boundary directly.

Typikon is designed as a cross-platform protocol. The project currently provides official adapters for **Python**, **Go**, and **TypeScript**; the set of implementations can grow over time through the community and future official bindings.

## Capabilities

| Capability | What it provides |
| --- | --- |
| Declarative `.typ` schemas | One source of truth for types and wire contracts |
| `struct` and data-bearing `enum` | Constructors with verifiable C-IDs |
| Flags and `#[guard(...)]` | Conditional fields without implicit nullable semantics |
| VarInt, collections, and maps | Compact and deterministic encoding |
| Layer negotiation | Explicit schema compatibility checks |
| Rust code generation | One implementation of encode/decode |
| Language adapters | Python via PyO3, Go via cgo, TypeScript via Node-API |
| Strict validation and limits | Predictable behavior on malformed input |

## Quick start

~~~bash
cargo run -- check examples/messenger.typ
# valid Layer 10: messenger.typ

cargo run -- compile examples/messenger.typ \
  --out-dir /tmp/typikon-messenger \
  --target python,golang,typescript

# public.typ can be recompiled without backend adapters;
# --target is optional
cargo run -- compile examples/messenger-10.public.typ \
  --out-dir /tmp/typikon-public

# compact public.typ when needed
cargo run -- compile examples/messenger.typ \
  --out-dir /tmp/typikon-public-compact \
  --public-format compact
~~~

Repository checks:

~~~bash
cargo fmt --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo check --manifest-path fuzz/Cargo.toml
~~~

## The schema language

~~~rust
#[version(10)]

#[flags(u16)]
enum UserFlags {
    IsBot = 0,
    HasAvatar = 1,
}

struct User {
    id: u64,
    name: String,
    flags: UserFlags,

    #[guard(flags.has_avatar)]
    avatar_url: String,
}

enum Message {
    Text { text: String },
    Image { data: Vec<u8> },
}
~~~

### Syntax and types

Every file starts with a required Layer, followed by flags, structs, and enums:

The `#[version(...)]` value is stored as `u16` and supports the inclusive range `0` to `65,535`.

```rust
#[version(10)]

#[flags(u8)]
enum MessageFlags {
    IsPinned = 0,
    HasReply = 1,
}

struct Message {
    id: u64,
    text: String,
    flags: MessageFlags,
    attachments: Vec<Attachment>,
}

struct Attachment {
    name: String,
    data: Vec<u8>,
}
```

Supported types:

| Category | Types |
| --- | --- |
| Boolean | `bool` |
| Unsigned integers | `u8`, `u16`, `u32`, `u64`, `u128` |
| Signed integers | `i8`, `i16`, `i32`, `i64`, `i128` |
| Floating point | `f32`, `f64` |
| Text and bytes | `String`, `Vec<u8>` |
| Collections | `Vec<T>`, `Map<K, V>` |
| User-defined types | `struct`, `enum`, and flags names |

`Vec<T>` can be nested: `Vec<Vec<u8>>`, `Map<String, Vec<Message>>`. A `Map<K, V>` key must be primitive, except `f32` and `f64`; pairs are encoded in sorted order. Violating this rule produces a semantic-validation error during `parse_schema`, never a parser panic.

For transport-level framing, `Encoder` also provides `write_vectored`: a header, packet, and trailer can be sent with one vectored write without an intermediate concatenation. The API is transport-neutral and works for TCP+TLS adapters; QUIC, WebSocket, and WebTransport can use the same packet buffer with their own message/frame boundaries.

### Flags and guard bits

Flags are enums annotated with `#[flags(u8)]`, `#[flags(u16)]`, `#[flags(u32)]`, `#[flags(u64)]`, or `#[flags(u128)]`. The value after `=` is a bit index:

```rust
#[flags(u16)]
enum UserFlags {
    IsBot = 0,
    IsVerified = 1,
    HasAvatar = 2,
}
```

`#[guard(flags.bit_name)]` connects a field to a previously declared flags field:

```rust
struct User {
    id: u64,
    flags: UserFlags,

    #[guard(flags.is_verified)]
    verified_at: u64,

    #[guard(flags.has_avatar)]
    avatar_url: String,
}
```

On the wire, a guard works as follows:

1. the flags field is encoded in its normal position;
2. the referenced bit is checked for each guarded field;
3. bit `1` — the field is encoded;
4. bit `0` — the field is omitted completely and occupies zero bytes.

The flags field must therefore appear before every field that depends on it. In generated Rust, guarded fields are `Option<T>`: `Some(value)` when the bit is set and `None` otherwise. Flags are not constructors and do not receive a Constructor ID.

During encoding, guarded-field presence is the source of truth: Rust, Go, TypeScript, and Python generators automatically set the linked bit for a present value and clear it when the value is absent. Callers do not need to synchronize `flags` manually; during decoding, the wire bit still determines whether the field is read.

Optionality is always explicit in Typikon; the schema has no separate `Option<T>` or `nullable<T>` syntax.

### Structs, enums, and unit enums

A `struct` describes one constructor:

```rust
struct User {
    id: u64,
    name: String,
}
```

A data-bearing `enum` describes multiple variants, each with its own Constructor ID:

```rust
enum Update {
    NewMessage { message: Message },
    MessageEdited { id: u64, text: String },
}
```

An enum without fields is an integer enum without a Constructor ID. Its values must be explicit and unique:

```rust
enum Presence {
    Offline = 0,
    Online = 1,
}
```

Trailing commas are allowed. Type, field, and variant names must be unique within their schema scope. Generated files must not be edited manually.

### Wire rules

- numbers use fixed-width little-endian encoding;
- strings and collections use VarInt for lengths or counts;
- `Map<K, V>` accepts primitive keys, sorts pairs, and rejects duplicate keys;
- the runtime packet limit is 4 MiB and the type nesting limit is 100;
- trailing bytes, malformed values, and truncated values are rejected.

### Constructor ID (C-ID)

In Typikon, a constructor is a concrete message type that can be encoded and decoded. Every `struct` and every data-bearing `enum` variant receives a Constructor ID automatically:

~~~text
AST constructor → canonical form → BLAKE3
              → first 16 hex characters → 8 raw bytes on the wire
~~~

The canonical form includes the constructor name, field order, types, and guard conditions. Source formatting and comments do not affect the C-ID; the Layer is not part of it so constructor identity remains shared across independent Layers. C-IDs currently occupy 64 bits: sufficient for beta and local schemas, but a public global registry would eventually need a namespace or an expanded format.

~~~text
[8 raw C-ID bytes][encoded fields]
~~~

The compiler emits `{name}-{layer}.public.typ`, a read-only schema passport with computed `#[cid(...)]` values and expanded struct formatting. It can be parsed again and compared with the compiler output.

## Demo: messenger

The main example is [`examples/messenger.typ`](examples/messenger.typ): a Layer 10 messenger schema with flags, presence, users, attachments, messages, and update events. The reproducible public artifact is [`messenger-10.public.typ`](examples/messenger-10.public.typ).

The CLI creates:

~~~text
messenger-10.rs              Rust wire core
messenger-10.public.typ      public schema with C-IDs
python.messenger-10.rs       Python native bridge
messenger_10.py              Python facade
golang.messenger-10.rs       Go native bridge
messenger-10.go / .h         Go API and C header
typescript.messenger-10.rs  TypeScript native bridge
messenger-10.ts              TypeScript facade
~~~

`fixtures/` contains small regression schemas, not standalone applications.

## Runtime and bindings

The generated Rust core is the only schema-specific implementation of binary encode/decode. The shared runtime lives in [`src/wire.rs`](src/wire.rs), [`src/codec.rs`](src/codec.rs), [`src/constructor.rs`](src/constructor.rs), and related modules.

- **Python** — an installable PyO3 package (`python -m pip install -e bindings/python`) with direct dict/list/scalar conversion through `pythonize`; packet ownership is available through `BorrowedPacket`.
- **Go** — cgo over the generated C ownership/error ABI.
- **TypeScript** — a Node-API native addon with a typed facade.

All adapters use one wire contract. Go is generated as a direct native encoder/decoder with borrowed and lazy views, TypeScript as a typed wire codec with lazy views, `BorrowedPacket`, and native Node-API `borrowBinary`/`borrowTyped`, and Python through direct PyO3 conversion with an owner-backed packet `memoryview`. Python typed borrowed `str`/list/dict views are intentionally not promised because their lifetime and ownership are controlled by Python's object model.

## Layers and compatibility

A Layer is an independent schema version, not an inherited set of changes:

~~~rust
let support = LayerSupport::new([6, 8, 10]);
assert_eq!(support.negotiate(8), Ok(8));
assert!(support.negotiate(9).is_err());
~~~

Only a Layer with a compiled backend is supported. Otherwise the runtime returns `LayerVersionNotSupported`. There is no Layer inheritance, `extends`, `@since`, or implicit version range. The separate `is_backward_compatible(old, new)` function is an opt-in developer migration audit; it does not make Layers an inheritance chain and is not part of Layer negotiation.

## Idea and intended scope

Typikon is an original schema-driven binary protocol implementation, inspired by **[TL](https://github.com/gotd/td)** and **[Protocol Buffers](https://github.com/protocolbuffers/protobuf)**. It is primarily aimed at messenger-like systems where compact messages, explicit Layer versions, stable Constructor IDs, conditional fields, and one wire contract across several languages matter.

Transport and application logic intentionally remain a separate layer. Typikon focuses on schemas, wire encoding, Layer compatibility, and generated adapters. The Python binding installs through `python -m pip install -e bindings/python`; the Go and TypeScript native crates build separately.

## What is actually tested

The Rust suite contains **67 tests: 62 unit tests and 5 integration tests** covering parsing and semantic validation, code generation, the CLI, Layer/C-ID handling, wire round-trips, limits, malformed input, maps, VarInt, borrowed views, language-view generation, vectored writes, randomized inputs, and FlatBuffers round-trip comparison.

Python/Go/TypeScript bindings, ownership/aliasing, lazy iteration, duplicate/unsorted maps, the cross-language round-trip, and semantic parity with FlatBuffers are also checked (`cargo test --test flatbuffers_comparison`, `(cd bindings/typescript && npm test)`, `go test ./bindings/go`, `./tests/cross_language_roundtrip.sh`, `./tests/generated_go_views.sh`). Benchmarks are available through `cargo bench --bench wire` and `cargo bench --bench compare`; they measure wire size, encoding/decoding, borrowed views, and allocations, but are not network benchmarks. Long-running validation is separate: `TYPIKON_STRESS_SECONDS=172800 ./tests/long_validation.sh`.

For a runtime-language comparison on the same messenger payload, use `TYPIKON_BENCH_ITERATIONS=10000 ./tests/language_benchmark.sh`; the Rust baseline comes from `cargo bench --bench compare`, Go additionally reports `allocs/op`, and Python/TypeScript report wall-clock `ns/op`.

## Repository layout

~~~text
src/                  parser, validator, runtime, compiler, codegen
examples/             messenger schema and public artifact
fixtures/             regression schemas
bindings/python/      PyO3 binding
bindings/go/          cgo binding and native crate
bindings/typescript/  Node-API binding and TS facade
tests/                CLI, artifact, and cross-language checks
CHANGELOG.md           history since the initial beta
~~~

Release history and current beta limitations are tracked in [`CHANGELOG.md`](CHANGELOG.md).

## License

Typikon is released under the [MIT License](LICENSE). Copyright © 2026 [ripcats](https://ripcats.t.me).
