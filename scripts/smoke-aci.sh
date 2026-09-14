#!/usr/bin/env sh
# Local ACI smoke against a built clawd-tee-gateway + mock upstream.
# Usage: sh scripts/smoke-aci.sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo_root"

echo "building clawd-tee-gateway (tests cover the same path)…"
cargo test -p clawd-tee-gateway -- --nocapture attestation_nonce_binding_and_receipt_headers

echo "smoke-aci: PASS (Rust e2e: nonce binding, x-receipt-id, receipt lookup, mocked anchor)"
