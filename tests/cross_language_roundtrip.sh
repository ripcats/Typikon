#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected="f4bd4aecfce83daf0700000000000000034164610000"
temp_dir="$(mktemp -d /tmp/typikon-cross-language.XXXXXX)"
trap 'rm -rf "$temp_dir"' EXIT

ln -s "$repo_dir/bindings/python/target/debug/libtypikon_python.so" "$temp_dir/typikon_python.so"
python_wire="$(PYTHONPATH="$temp_dir" python3 -c 'import typikon_python; print(typikon_python.User(b"{\"id\":7,\"name\":\"Ada\",\"flags\":0}").encode().hex())')"
test "$python_wire" = "$expected"

cp "$repo_dir/bindings/typescript/native/target/debug/libtypikon_typescript_native.so" "$temp_dir/typikon_typescript_native.node"
node_wire="$(node - "$temp_dir/typikon_typescript_native.node" <<'JS'
const n = require(process.argv[2]);
process.stdout.write(n.encodeJson(10, "user", Buffer.from('{"id":7,"name":"Ada","flags":0}')).toString("hex"));
JS
)"
test "$node_wire" = "$expected"

(cd "$repo_dir/bindings/go" && go test ./...)
printf 'cross-language round-trip: PASS\n'
