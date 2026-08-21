#!/usr/bin/env bash
set -uo pipefail

duration_seconds="${TYPIKON_STRESS_SECONDS:-172800}"
log_file="${TYPIKON_STRESS_LOG:-/tmp/typikon-long-validation.log}"
started_at="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
deadline=$(( $(date +%s) + duration_seconds ))

mkdir -p "$(dirname "$log_file")"
exec > >(tee -a "$log_file") 2>&1

echo "Typikon long validation started=$started_at duration_seconds=$duration_seconds pid=$$"
iteration=0
while (( $(date +%s) < deadline )); do
    iteration=$((iteration + 1))
    echo "iteration=$iteration started=$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    cargo test --release --all-targets || exit 1
    cargo test --release --manifest-path bindings/typescript/native/Cargo.toml || exit 1
    (cd bindings/typescript && npm test) || exit 1
    ./tests/cross_language_roundtrip.sh || exit 1

    echo "iteration=$iteration passed=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
done

echo "Typikon long validation finished=$(date -u +%Y-%m-%dT%H:%M:%SZ) iterations=$iteration"
