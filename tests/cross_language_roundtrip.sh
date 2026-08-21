#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected="acb38da67a712058070000000000000003616461034164610000000000000000000000"
json='{"id":7,"username":"ada","display_name":"Ada","flags":0,"presence":"Online","roles":[]}'
temp_dir="$(mktemp -d /tmp/typikon-cross-language.XXXXXX)"
trap 'rm -rf "$temp_dir"' EXIT

cargo build --quiet --manifest-path "$repo_dir/bindings/python/Cargo.toml"
cargo build --quiet --manifest-path "$repo_dir/bindings/typescript/native/Cargo.toml"

ln -s "$repo_dir/bindings/python/target/debug/libtypikon_python.so" "$temp_dir/typikon_python.so"
python_wire="$(TYPIKON_VALUE="$json" PYTHONPATH="$temp_dir" python3 -c 'import json, os, typikon_python; print(typikon_python.encode_user(json.loads(os.environ["TYPIKON_VALUE"])).hex())')"
test "$python_wire" = "$expected"

cp "$repo_dir/bindings/typescript/native/target/debug/libtypikon_typescript_native.so" "$temp_dir/typikon_typescript_native.node"
node_wire="$(TYPIKON_VALUE="$json" node - "$temp_dir/typikon_typescript_native.node" <<'JS'
const n = require(process.argv[2]);
const json = Buffer.from(process.env.TYPIKON_VALUE);
const wire = n.encodeJson(10, "user", json);
const decoded = JSON.parse(n.decodeJson(10, "user", wire).toString());
if (decoded.id !== 7 || decoded.username !== "ada" || decoded.display_name !== "Ada") process.exit(1);
process.stdout.write(wire.toString("hex"));
JS
)"
test "$node_wire" = "$expected"

(cd "$repo_dir/bindings/go" && go test ./...)
printf 'Python / TypeScript / Go cross-language round-trip: PASS\n'
