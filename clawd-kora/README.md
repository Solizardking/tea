# Clawd Kora relayer + x402 facilitator

Gasless Solana signing for Clawd services. Users pay in `$CLAWD`
(`8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump`); Kora pays SOL network fees.

```
Client ──HTTP 402──▶ x402-proxy-template ──/verify /settle──▶ facilitator ──JSON-RPC──▶ Kora RPC ──▶ Solana
                         │                                         │
                         └── origin: clawd-tee-gateway             └── kora.toml allowlists $CLAWD
```

Three units (do not put Kora private keys in the Worker):

| Unit | Path | Role |
| --- | --- | --- |
| Kora operator | `kora.toml`, `signers.toml` | Policy + memory signer |
| Facilitator | `facilitator/` | x402 `GET /supported`, `POST /verify`, `POST /settle` |
| Payment proxy | `../x402-proxy-template` | HTTP 402 gate + JWT cookie after settlement |

## Requirements

- [kora-cli 2.0.5](https://github.com/solana-foundation/kora/releases/tag/v2.0.5) (`cargo install kora-cli --version 2.0.5`)
- Node.js 22+
- A funded Kora signer (devnet SOL) for live `signAndSendTransaction`

## Configure

```bash
cp .env.example .env
# set KORA_SIGNER_PRIVATE_KEY (base58 64-byte secret)
```

`allowed_spl_paid_tokens` in `kora.toml` must include the `$CLAWD` mint. `price_source = "Mock"` is required for local/dev — Jupiter will not price this pump mint without extra oracle setup.

## Run

```bash
# Terminal 1 — Kora JSON-RPC (default :8080)
./scripts/start-kora.sh

# Terminal 2 — x402 facilitator (default :3000)
cd facilitator && npm install && npm start

# Terminal 3 — payment proxy (default :8787)
cd ../x402-proxy-template && npm run dev
```

Optional: initialize ATAs for the signer so it can receive `$CLAWD`:

```bash
kora --config kora.toml --rpc-url "$RPC_URL" rpc initialize-atas --signers-config signers.toml
```

## Check

```bash
curl -s http://127.0.0.1:8080 -H 'content-type: application/json' \
  -H "x-api-key: $KORA_API_KEY" \
  -d '{"jsonrpc":"2.0","id":1,"method":"getConfig","params":{}}'

curl -s http://127.0.0.1:3000/supported
curl -s http://127.0.0.1:8787/__x402/health
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8787/__x402/protected
# 402
```

Public Clawd health stays unpaid: gateway `GET /health` (proxied) and proxy `GET /__x402/health`.
