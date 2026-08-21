# Go bridge

This package uses cgo to call a schema-specific Rust bridge. The Rust-generated `NAME-VERSION.rs` backend remains the only wire implementation; the `golang.NAME-VERSION.rs` file only exposes the C ownership/error ABI for Go.

From the repository root:

```bash
cargo build --manifest-path bindings/go/native/Cargo.toml
cargo run -- compile examples/messenger.typ --out-dir /tmp/typikon-messenger
go test ./bindings/go
```

The CLI output contains `messenger-10.go` and `messenger-10.h`. Put those files beside the Go package when building a typed schema adapter and link `libtypikon_go_native.so` (or the static library) produced by the native crate. The generated API owns Rust result buffers through `typikon_free_bytes`; failed encode/decode calls return the native error text.

The installed environment used for the repository checks does not include the Go toolchain, so `cargo build` for the native cgo library is verified here while `go test` must be run on a machine with Go and cgo enabled.
