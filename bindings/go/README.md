# Go bridge

This package uses cgo to call a schema-specific Rust bridge. The Rust-generated `NAME-VERSION.rs` backend remains the only wire implementation; the `golang.NAME-VERSION.rs` file only exposes the C ownership/error ABI for Go.

From the repository root:

```bash
cargo build --manifest-path bindings/go/native/Cargo.toml
cargo run -- compile examples/messenger.typ --out-dir /tmp/typikon-messenger
go test ./bindings/go
```

The CLI output contains `messenger-10.go` and `messenger-10.h`. Put those files beside the Go package when building a typed schema adapter and link `libtypikon_go_native.so` (or the static library) produced by the native crate. The generated API owns Rust result buffers through `typikon_free_bytes`; failed encode/decode calls return the native error text.

The checked-in `messenger-10.go` facade and `messenger-10.h` header are generated from the public messenger schema. The repository test suite builds both Rust libraries and verifies the Rust/Go golden wire round-trip with `go test ./bindings/go`.
