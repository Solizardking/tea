# Clawd x402 facilitator

Thin HTTP adapter from x402 (`/supported`, `/verify`, `/settle`) onto Kora JSON-RPC.

| Endpoint | Kora method | Broadcasts? |
| --- | --- | --- |
| `GET /supported` | `getPayerSigner` | no |
| `POST /verify` | `signTransaction` | no |
| `POST /settle` | `signAndSendTransaction` | yes |

Rejects non-Solana `paymentRequirements.network` (`solana-devnet`, `solana`, or CAIP-2 `solana:…`).

```bash
npm install
KORA_RPC_URL=http://127.0.0.1:8080 NETWORK=solana-devnet npm start
```

Tests drive the real handlers against a stub Kora JSON-RPC (no mocked facilitator, no hardcoded signatures):

```bash
npm test
```
