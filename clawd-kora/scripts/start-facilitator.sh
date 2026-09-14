#!/usr/bin/env bash
# Start the Clawd x402 facilitator (maps /verify /settle /supported onto Kora).
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT/facilitator"

if [[ -f "$ROOT/.env" ]]; then
  set -a
  # shellcheck disable=SC1091
  source "$ROOT/.env"
  set +a
fi

export KORA_RPC_URL="${KORA_RPC_URL:-http://127.0.0.1:8080}"
export FACILITATOR_PORT="${FACILITATOR_PORT:-3000}"
export NETWORK="${NETWORK:-solana-devnet}"

exec npm start
