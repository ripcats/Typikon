# Go bridge

Typed Go adapters use a native Go wire codec generated from the schema. Encode/decode write and read Typikon bytes directly, without a JSON representation or cgo round-trip. cgo remains only for ABI negotiation and borrowed wire validation.

From the repository root:

```bash
cargo build --manifest-path bindings/go/native/Cargo.toml
cargo run -- generate all examples/messenger.typ --out-dir /tmp/typikon-messenger
go test ./bindings/go
```

The CLI output contains `messenger-10.go` and `messenger-10.h`. Put those files beside the Go package when building a typed schema adapter. `Validate<Type>(wire)` runs checked borrowed validation against the caller-owned `[]byte` without materializing a decoded object.

The checked-in `messenger-10.go` facade and `messenger-10.h` header are generated from the public messenger schema. Generated adapters also expose `Borrow<Type>Lazy` views whose `Len`/`At` methods scan the caller-owned packet without allocating collection metadata. The repository test suite builds both Rust libraries and verifies the Rust/Go golden wire round-trip with `go test ./bindings/go`.
