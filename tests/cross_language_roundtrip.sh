#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected="acb38da67a712058070000000000000003616461034164610000000000000000000000"
json='{"id":7,"username":"ada","display_name":"Ada","flags":0,"presence":"Online","roles":[]}'
temp_dir="$(mktemp -d /tmp/typikon-cross-language.XXXXXX)"
trap 'rm -rf "$temp_dir"' EXIT

cargo build --quiet --manifest-path "$repo_dir/bindings/python/Cargo.toml"
cargo build --quiet --manifest-path "$repo_dir/bindings/typescript/native/Cargo.toml"
cargo run --quiet --manifest-path "$repo_dir/Cargo.toml" -- compile "$repo_dir/examples/messenger.typ" --out-dir "$temp_dir/generated" --target typescript
cargo run --quiet --manifest-path "$repo_dir/Cargo.toml" -- compile "$repo_dir/examples/messenger.typ" --out-dir "$temp_dir/generated-go" --target golang
gofmt -w "$temp_dir/generated-go/messenger-10.go"
mkdir -p "$temp_dir/generated-js"
"$repo_dir/bindings/typescript/node_modules/.bin/tsc" --target ES2022 --module commonjs --outDir "$temp_dir/generated-js" "$temp_dir/generated/messenger-10.ts"

ln -s "$repo_dir/bindings/python/target/debug/libtypikon_python.so" "$temp_dir/typikon_python.so"
python_wire="$(TYPIKON_VALUE="$json" PYTHONPATH="$temp_dir" python3 -c 'import json, os, typikon_python; print(typikon_python.encode_user(json.loads(os.environ["TYPIKON_VALUE"])).hex())')"
test "$python_wire" = "$expected"

cp "$repo_dir/bindings/typescript/native/target/debug/libtypikon_typescript_native.so" "$temp_dir/typikon_typescript_native.node"
typed_node_wire="$(node - "$temp_dir/generated-js/messenger-10.js" <<'JS'
const m = require(process.argv[2]);
const wire = m.encodeUser({id: 7, username: "ada", display_name: "Ada", flags: 0, presence: "Online", roles: []});
process.stdout.write(Buffer.from(wire).toString("hex"));
JS
)"
test "$typed_node_wire" = "$expected"
node - "$temp_dir/generated-js/messenger-10.js" <<'JS'
const m = require(process.argv[2]);
const wire = m.encodeUser({id: 7, username: "ada", display_name: "Ada", flags: 0, presence: "Online", roles: ["admin"]});
const view = m.decodeUserView(wire);
if (!(view.username instanceof Uint8Array) || new TextDecoder().decode(view.username) !== "ada") process.exit(1);
const pos = wire.indexOf(97);
wire[pos] = 122;
if (new TextDecoder().decode(view.username) !== "zda") process.exit(1);
try { m.decodeUserView(wire.slice(0, -1)); process.exit(1); } catch (_) {}
JS
node_wire="$(node - "$temp_dir/typikon_typescript_native.node" <<'JS'
const n = require(process.argv[2]);
const wire = Buffer.from("acb38da67a712058070000000000000003616461034164610000000000000000000000", "hex");
const decoded = n.decodeBinary(10, "user", n.encodeBinary(10, "user", wire));
if (!decoded.equals(wire)) process.exit(1);
process.stdout.write(wire.toString("hex"));
JS
)"
test "$node_wire" = "$expected"

(cd "$repo_dir/bindings/go" && go test ./...)
printf 'Python / TypeScript / Go cross-language round-trip: PASS\n'
