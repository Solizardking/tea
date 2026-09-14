#!/usr/bin/env bash
# Start the Clawd Kora RPC (JSON-RPC paymaster) from clawd-kora/.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

if [[ -f .env ]]; then
  set -a
  # shellcheck disable=SC1091
  source .env
  set +a
fi

if [[ -z "${KORA_SIGNER_PRIVATE_KEY:-}" ]]; then
  echo "KORA_SIGNER_PRIVATE_KEY is not set. Copy .env.example to .env and add a base58 secret key." >&2
  exit 1
fi

RPC_URL="${RPC_URL:-https://api.devnet.solana.com}"
KORA_PORT="${KORA_PORT:-8080}"

exec kora \
  --config "$ROOT/kora.toml" \
  --rpc-url "$RPC_URL" \
  rpc start \
  --signers-config "$ROOT/signers.toml" \
  --port "$KORA_PORT"
