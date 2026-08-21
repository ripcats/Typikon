#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
iterations="${TYPIKON_BENCH_ITERATIONS:-10000}"
temp_dir="$(mktemp -d /tmp/typikon-language-bench.XXXXXX)"
go_bench="$repo_dir/bindings/go/language_benchmark_test.go"
trap 'rm -rf "$temp_dir"; rm -f "$go_bench"' EXIT

cargo build --quiet --manifest-path "$repo_dir/bindings/python/Cargo.toml"
cargo build --quiet --manifest-path "$repo_dir/bindings/typescript/native/Cargo.toml"
cargo build --quiet --manifest-path "$repo_dir/bindings/go/native/Cargo.toml"
cargo build --quiet --manifest-path "$repo_dir/Cargo.toml"

cargo run --quiet --manifest-path "$repo_dir/Cargo.toml" -- compile "$repo_dir/examples/messenger.typ" --out-dir "$temp_dir/generated" --target python,typescript
cargo run --quiet --manifest-path "$repo_dir/Cargo.toml" -- compile "$repo_dir/examples/messenger.typ" --out-dir "$temp_dir/generated-go" --target golang
gofmt -w "$temp_dir/generated-go/messenger-10.go"

if [[ ! -x "$repo_dir/bindings/typescript/node_modules/.bin/tsc" ]]; then
    (cd "$repo_dir/bindings/typescript" && npm ci --ignore-scripts)
fi
mkdir -p "$temp_dir/generated-js"
"$repo_dir/bindings/typescript/node_modules/.bin/tsc" --target ES2022 --module commonjs --outDir "$temp_dir/generated-js" "$temp_dir/generated/messenger-10.ts"
ln -s "$repo_dir/bindings/python/target/debug/libtypikon_python.so" "$temp_dir/typikon_python.so"
cp "$repo_dir/bindings/typescript/native/target/debug/libtypikon_typescript_native.so" "$temp_dir/typikon_typescript_native.node"

cat > "$temp_dir/python_bench.py" <<'PY'
import json
import os
import time
import messenger_10

iterations = int(os.environ["TYPIKON_BENCH_ITERATIONS"])
value = {"id": 1, "chat_id": 2, "sender": {"id": 7, "username": "ada", "display_name": "Ada", "flags": 0, "presence": "Online", "roles": ["admin", "moderator"]}, "text": "hello", "attachments": [{"id": 3, "name": "photo", "mime_type": "image/jpeg", "size": 4096}], "metadata": {"client": "web", "locale": "en"}}
wire = messenger_10.encode_message(value)
for _ in range(100):
    messenger_10.encode_message(value)
    messenger_10.decode_message(wire)
start = time.perf_counter_ns()
for _ in range(iterations):
    messenger_10.encode_message(value)
encode_ns = (time.perf_counter_ns() - start) / iterations
start = time.perf_counter_ns()
for _ in range(iterations):
    messenger_10.decode_message(wire)
decode_ns = (time.perf_counter_ns() - start) / iterations
start = time.perf_counter_ns()
for _ in range(iterations):
    messenger_10.borrowed_packet_message(wire)
borrow_ns = (time.perf_counter_ns() - start) / iterations
print(f"language=python bytes={len(wire)} encode_ns={encode_ns:.1f} decode_ns={decode_ns:.1f} borrowed_validate_ns={borrow_ns:.1f}")
PY

PYTHONPATH="$temp_dir/generated:$temp_dir" TYPIKON_BENCH_ITERATIONS="$iterations" python3 "$temp_dir/python_bench.py"

node - "$temp_dir/generated-js/messenger-10.js" "$iterations" <<'JS'
const m = require(process.argv[2]);
const iterations = Number(process.argv[3]);
const value = {id: 1, chat_id: 2, sender: {id: 7, username: "ada", display_name: "Ada", flags: 0, presence: "Online", roles: ["admin", "moderator"]}, text: "hello", attachments: [{id: 3, name: "photo", mime_type: "image/jpeg", size: 4096}], metadata: {client: "web", locale: "en"}};
const wire = m.encodeMessage(value);
for (let i = 0; i < 100; i++) { m.encodeMessage(value); m.decodeMessage(wire); }
let start = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) m.encodeMessage(value);
const encodeNs = Number(process.hrtime.bigint() - start) / iterations;
start = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) m.decodeMessage(wire);
const decodeNs = Number(process.hrtime.bigint() - start) / iterations;
start = process.hrtime.bigint();
for (let i = 0; i < iterations; i++) m.decodeMessageLazyView(wire);
const lazyNs = Number(process.hrtime.bigint() - start) / iterations;
console.log(`language=typescript bytes=${wire.length} encode_ns=${encodeNs.toFixed(1)} decode_ns=${decodeNs.toFixed(1)} lazy_view_ns=${lazyNs.toFixed(1)}`);
JS

cat > "$go_bench" <<'GO'
package typikon

import "testing"

var languageBenchValue = Message{Id: 1, ChatId: 2, Sender: User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online"), Roles: []string{"admin", "moderator"}}, Text: "hello", Attachments: []Attachment{{Id: 3, Name: "photo", MimeType: "image/jpeg", Size: 4096}}, Metadata: map[string]string{"client": "web", "locale": "en"}}
var languageBenchWire, _ = EncodeMessage(languageBenchValue)

func BenchmarkLanguageEncode(b *testing.B) { for i := 0; i < b.N; i++ { _, _ = EncodeMessage(languageBenchValue) } }
func BenchmarkLanguageDecode(b *testing.B) { for i := 0; i < b.N; i++ { _, _ = DecodeMessage(languageBenchWire) } }
func BenchmarkLanguageBorrowed(b *testing.B) { for i := 0; i < b.N; i++ { _, _ = BorrowMessageLazy(languageBenchWire) } }
GO
(cd "$repo_dir/bindings/go" && go test -run '^$' -bench '^BenchmarkLanguage' -benchmem .) | sed -n '/BenchmarkLanguage/p'

printf 'language=rust baseline=run cargo bench --bench compare (same messenger payload profile)\n'
