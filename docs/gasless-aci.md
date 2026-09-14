# Gasless $CLAWD + Private AI Gateway (public)

Users pay in `$CLAWD` (`8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump`); Kora pays SOL network fees.

Animated HTML: [/agent/gasless-aci.html](/agent/gasless-aci.html)

```
Client ──HTTP 402──▶ x402-proxy ──/verify /settle──▶ facilitator ──JSON-RPC──▶ Kora ──▶ Solana
```

| Unit | Path | Role |
| --- | --- | --- |
| Kora operator | `kora.toml`, `signers.toml` | Policy + memory signer |
| Facilitator | `facilitator/` | `/supported`, `/verify`, `/settle` |
| Payment proxy | `x402-proxy-template` | HTTP 402 + JWT cookie |

Do **not** put Kora private keys in the Worker.

Public unpaid health: gateway `GET /health`, proxy `GET /__x402/health`.

Repo: https://github.com/Solizardking/tea
