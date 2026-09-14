# Security Policy

## Reporting Vulnerabilities

Please report suspected vulnerabilities privately before opening a public issue.
Include affected commit, component, reproduction steps, and any proof impact you
can share safely. Do not include private keys, API tokens, wallet seed material,
or production request payloads in reports.

## Secrets Policy

Never commit:

- Solana keypair JSON files or key arrays
- `.env` files
- PEM/private key material
- upstream provider API keys
- RPC URLs containing embedded credentials
- generated `target/`, `dist/`, or `node_modules/` artifacts

Use `.env.example` for names and placeholders only. Keep deploy credentials in
the platform secret manager, and rotate any key that has ever been committed to
Git history.

## Production Key Handling

`SOLANA_KEYPAIR_PATH` or `SOLANA_KEYPAIR_JSON` controls the fee payer used for
proof anchoring and SAS issuance. Treat it as a wallet with spend authority.

`SIGNING_KEY_PEM_PATH` or `SIGNING_KEY_PEM` controls the secp256k1 proof-signing
identity. In production, keep this key in TEE-backed, sealed, or encrypted
storage. Ephemeral signing keys are acceptable only for local development and
will change proof identity on restart.

## Public Deployment Checklist

- Rotate any key that was ever committed, even if later removed.
- Rewrite or replace Git history before publishing if secrets or build artifacts
  were previously tracked.
- Keep `ALLOW_CLIENT_AUTHORIZATION=false` for public gateways unless the gateway
  is intentionally caller-keyed.
- Keep `ALLOW_INSECURE_UPSTREAM=false` outside local loopback development.
- Set `ADMIN_TOKEN` before enabling SAS credential mutation routes.
- Set a concrete `CORS_ALLOWED_ORIGIN` only for browser clients you control.
- Use `VERIFICATION_LEVEL=1` only when the native secp256k1 instruction signs
  the exact `signed_payload_hash` stored in `StoreProofV2`.
- ACI receipts contain hashes only and expire from memory (`RECEIPT_TTL_SECS`).
  Do not treat them as a durable log; the durable record is the Solana PDA.
