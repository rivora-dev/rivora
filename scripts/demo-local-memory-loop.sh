#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SCENARIO="${1:-${RIVORA_DEMO_SCENARIO:-basic}}"

case "$SCENARIO" in
  basic | checkout-incident | release-regression | workflow-failure) ;;
  *)
    printf 'Unknown demo scenario: %s\n' "$SCENARIO" >&2
    printf 'Supported scenarios: basic, checkout-incident, release-regression, workflow-failure\n' >&2
    exit 2
    ;;
esac

run_rivora() {
  if [[ -n "${RIVORA_BIN:-}" ]]; then
    "$RIVORA_BIN" "$@"
  else
    cargo run -q --manifest-path "$ROOT_DIR/Cargo.toml" -p rivora-cli -- "$@"
  fi
}

printf 'Rivora packaged demo script\n'

if [[ "${RIVORA_DEMO_KEEP:-0}" == "1" ]]; then
  run_rivora demo --scenario "$SCENARIO" --keep
else
  run_rivora demo --scenario "$SCENARIO"
fi
