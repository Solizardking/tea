# CLAWD TEE as a RedPill-compatible confidential provider

This gateway is an **OpenAI-compatible aggregator** that publishes ACI
(attested confidential inference) artifacts and settles each inference as a
**TeeProofV2 PDA on Solana**. Dark Pool and other SVM clients should treat it
as a replaceable RedPill-style provider: verify attestation + receipt off-chain,
then confirm the on-chain digest.

Gateways compute. Solana settles. Digests only — never GPU blobs or prompts.

## Settlement policy

**Solana SVM settles; TEEs compute; digests only — never put GPU on-chain.**

On-chain proof store and lookup default to Solana (`solana` → `devnet`, or `mainnet-beta`) via TeeProof / TeeProofV2 and `clawd-verifier`. Ethereum, Sepolia, Holesky, and Automata are not production defaults; they require `--legacy-evm`.

## Public endpoints

| Method | Path | Role |
| --- | --- | --- |
| `GET` | `/health` | Liveness, `workload_id`, keyset digest, whether Solana anchoring is enabled |
| `GET` | `/v1/aci/attestation?nonce=<64-hex>` | ACI attestation report bound to the caller nonce |
| `GET` | `/v1/attestation` | Alias of the ACI report (also `/v1/attestation/report`) |
| `POST` | `/v1/chat/completions` | Proxied chat + receipt headers + TeeProofV2 attempt |
| `POST` | `/v1/completions` | Same for legacy completions |
| `POST` | `/v1/embeddings` | Same for embeddings |
| `GET` | `/v1/aci/receipts/{id}` | Signed receipt (`rcpt-…` or upstream `id`) |
| `GET` | `/v1/proof/:payload_hash` | On-chain TeeProofV2 by `signed_payload_hash` |

Inference responses always include:

| Header | Meaning |
| --- | --- |
| `x-receipt-id` | Lookup id for `GET /v1/aci/receipts/{id}` |
| `x-aci-identity` | Stable `workload_id` (`sha256:…`) |
| `x-aci-keyset-digest` | JCS digest of the published keyset |
| `x-aci-version` | `aci/1` |

The JSON body still carries `_clawd` (hashes, signature, Solana PDA / tx) for
existing CLAWD / Dark Pool clients.

## Client verification (attestation + receipt + PDA)

```bash
export BASE=https://clawd-tee-gateway-production.up.railway.app   # or http://127.0.0.1:8080 for local
export NONCE=$(openssl rand -hex 32)

# 1. Fresh attestation. report_data[0..32] = sha256(ACI statement).
curl -sS "$BASE/v1/aci/attestation?nonce=$NONCE" -o report.json
jq '{api_version, workload_id, workload_keyset_digest,
     report_data: .attestation.report_data,
     tee: .attestation.tee_type,
     tdx_backend: .attestation.evidence.tdx_backend}' report.json

# 2. Inference — capture receipt id.
curl -sS -D headers.txt "$BASE/v1/chat/completions" \
  -H "Content-Type: application/json" \
  -d '{"model":"openai/gpt-4o-mini","messages":[{"role":"user","content":"ping"}]}' \
  -o response.json
export RECEIPT_ID=$(grep -i '^x-receipt-id' headers.txt | tr -d '\r' | awk '{print $2}')

# 3. Signed receipt. event_log includes upstream.verified and solana.tee_proof_v2.
curl -sS "$BASE/v1/aci/receipts/$RECEIPT_ID" -o receipt.json
jq '{receipt_id, workload_id, workload_keyset_digest, endpoint,
     events: [.event_log[].type]}' receipt.json

# 4. On-chain PDA (when SOLANA_KEYPAIR_* is set).
PAYLOAD=$(jq -r '._clawd.signed_payload_hash' response.json)
curl -sS "$BASE/v1/proof/$PAYLOAD"
# Explorer (Orb): https://orbmarkets.io/address/<solana_proof_address>
```

Checks a relying party should perform:

1. Rebuild the ACI statement
   `{"keyset_digest":"<digest>","nonce":"<nonce>","purpose":"aci.report_data.v1"}`
   and confirm `sha256(statement)` equals `attestation.report_data[0..32]`.
2. Recompute `sha256(JCS(workload_keyset))` and match `workload_keyset_digest`.
3. Confirm receipt `workload_id` / `workload_keyset_digest` match the report.
4. Verify `signature` under `receipt_signing_keys` (`ecdsa-secp256k1` /
   `receipt-v1`): 65-byte recoverable secp256k1 over `keccak256(JCS(receipt
   without signature))`.
5. Confirm `request.received.body_hash` and `response.returned.body_hash`
   match the exact bytes you sent and received.
6. Read `upstream.verified`. This aggregator does **not** independently
   verify upstream TEEs; expect `result=failed`, `required=false` unless you
   run a TEE-verified upstream and we observed its ACI headers.
7. When `SOLANA_KEYPAIR_*` is configured, `_clawd.solana_tx_sig` is non-empty
   and `_clawd.attestation_report_hash` is not 32 zero bytes. Look up the PDA
   seeded by `["tee_proof_v2", signed_payload_hash]`.

## Receipt retention

Receipts are **in-memory with TTL**. Defaults:

| Variable | Default | Notes |
| --- | --- | --- |
| `RECEIPT_TTL_SECS` | `3600` (1 hour) | Expired ids return `404 not_found` |
| `RECEIPT_MAX_ENTRIES` | `10000` | Oldest entries evicted when full |

Clients should fetch the receipt immediately after the inference response.
This is not a durable public transparency log. The durable record is the
Solana TeeProofV2 PDA (digests only).

## Operate as a RedPill-style provider (Railway)

Hosted BASE: `https://clawd-tee-gateway-production.up.railway.app`

The service is a Docker build of `clawd-tee-gateway/` (healthcheck `GET /health`).
Railway injects `PORT`; the binary binds `BIND_ADDR` (default `0.0.0.0`) and
honors `PORT` (default `8080`). Do not pin `PORT` in service variables.

IaC lives at [`.railway/railway.ts`](./.railway/railway.ts). Secrets stay in
Railway variables (`preserve()` in IaC) — never commit them.

```bash
cd clawd-tee-gateway
# Public env is already on the Railway service (UPSTREAM_URL, Solana RPC,
# REDPILL_PROGRAM_ID, verification/TTL knobs, WORKLOAD_SUBJECT, SOURCE_REPO_URL,
# RUST_LOG). Set secrets in the Railway dashboard or:
railway variable set UPSTREAM_API_KEY --stdin --service clawd-tee-gateway
railway variable set ADMIN_TOKEN --stdin --service clawd-tee-gateway
# optional:
#   railway variable set SOLANA_KEYPAIR_JSON --stdin --service clawd-tee-gateway
#   railway variable set SIGNING_KEY_PEM --stdin --service clawd-tee-gateway
#   railway variable set NVIDIA_API_KEY --stdin --service clawd-tee-gateway
railway up --path-as-root --detach -m "deploy clawd-tee-gateway"
```

Verifier / client check against the hosted gateway:

```bash
npx clawd-verifier health --gateway-url https://clawd-tee-gateway-production.up.railway.app
# local loopback remains valid:
# npx clawd-verifier health --gateway-url http://127.0.0.1:8081
```

The ACI/OpenAI HTTP surface is unchanged (`/health`, `/v1/aci/attestation`,
`/v1/chat/completions`, receipts, `/v1/proof/:payload_hash`) so `clients/`,
`clawd-verifier`, OP-TEE host mapping, and `private-ai-gateway-main` consumers
keep the same protocol.

Required for a public confidential-provider claim:

| Secret / env | Required | Why |
| --- | --- | --- |
| `UPSTREAM_API_KEY` | yes* | Upstream inference (`*` unless `ALLOW_CLIENT_AUTHORIZATION=true`) |
| `ADMIN_TOKEN` | yes for SAS admin routes | `/v1/credential/*` stay dead without it |
| `SOLANA_KEYPAIR_JSON` or `SOLANA_KEYPAIR_PATH` | yes to settle | Fee payer for `StoreProofV2`. If unset, the gateway **logs a warning** and still issues receipts / computes the PDA |
| `NVIDIA_API_KEY` | optional | NRAS GPU attestation. Digests only; GPU evidence never goes on-chain |
| `SIGNING_KEY_PEM` / `SIGNING_KEY_PEM_PATH` | production | Persist the secp256k1 TeeProof signer across restarts |
| `SOURCE_REPO_COMMIT` / `SOURCE_IMAGE_DIGEST` | production | ACI provenance labels (not a trust anchor unless measured) |

Railway service variables already include `UPSTREAM_URL = "https://openrouter.ai/api"`
and the deployed `REDPILL_PROGRAM_ID`.

### What still needs real TDX / Railway secrets

- **`/dev/tdx_guest`**: Railway is not a TDX host. Without the guest device the
  gateway uses the stub TDX backend. Quotes are deterministic placeholders.
  `attestation.evidence.tdx_backend` will be `"stub"`. Production confidential
  claims need a TDX VM (or equivalent) so `tdx_backend=real` and DCAP can
  verify the quote.
- **Railway secrets** listed above. The image will boot without Solana / NVIDIA
  keys; it will not silently pretend it anchored.
- **Intel DCAP collateral** and a reviewed compose / image digest. This
  repo does not run a full dstack KMS or RTMR3 replay.
- **`GET /v1/aci/sessions/{id}`** is not implemented. Receipts record
  `upstream.verified` without a content-addressed session document.
- **E2EE v2** and TLS SPKI pinning in the keyset are not implemented
  (`supported_e2ee_versions: []`, empty `tls_public_keys`).

## Deploy on Render

The root [`render.yaml`](./render.yaml) Blueprint deploys this repo’s
`clawd-tee-gateway/` (not `zk-router` / `web/solana-tee/...`).

| Blueprint field | Value |
| --- | --- |
| Repo | `https://github.com/Solizardking/clawd-tee` |
| `rootDir` | `clawd-tee-gateway` |
| `dockerfilePath` | `./Dockerfile` (relative to `rootDir`) |
| `dockerContext` | `.` (the gateway directory) |
| `healthCheckPath` | `/health` |

Create the Blueprint from the repo root, then set dashboard secrets. Do not
commit key material.

| Secret / env | Required | Why |
| --- | --- | --- |
| `UPSTREAM_API_KEY` | yes* | Upstream inference (`*` unless `ALLOW_CLIENT_AUTHORIZATION=true`) |
| `ADMIN_TOKEN` | yes for SAS admin routes | `/v1/credential/*` stay dead without it |
| `SOLANA_KEYPAIR_JSON` or `SOLANA_KEYPAIR_PATH` | optional | Fee payer for `StoreProofV2`. Env JSON is the usual Render path; or upload a secret file and keep `SOLANA_KEYPAIR_PATH=/etc/secrets/devnet-payer.json` |
| `SIGNING_KEY_PEM` or `SIGNING_KEY_PEM_PATH` | optional / production | Persist the secp256k1 TeeProof signer. Env PEM, or a secret file at `SIGNING_KEY_PEM_PATH=/etc/secrets/tee-signing-key.pem` |

Non-secret Blueprint defaults (`UPSTREAM_URL`, `SOLANA_RPC_URL`,
`REDPILL_PROGRAM_ID`, policy knobs, `RUST_LOG`) stay in `render.yaml`.
`UPSTREAM_API_KEY` and `ADMIN_TOKEN` are `sync: false` stubs — fill them in
the Render dashboard.

Render remains an alternate Blueprint, not a Railway replacement.
The image still boots without Solana / signing keys; it will not silently
pretend it anchored.

## Compatibility matrix

| Surface | private-ai-gateway / RedPill ACI | CLAWD TEE (this repo) |
| --- | --- | --- |
| `GET /v1/aci/attestation?nonce=` | Required; 64 lowercase hex | Same; rejects anything else with 400 |
| ACI statement → `report_data` | `sha256(statement)`, zero-padded to 64 | Same (pinned to ACI test vectors) |
| Workload keyset + JCS digest | `ed25519` baseline (may list more) | `ecdsa-secp256k1` (`receipt-v1`) — same key as TeeProofV2; persist with `SIGNING_KEY_PEM` |
| Response headers | `x-receipt-id`, `x-aci-identity`, `x-aci-keyset-digest`, `x-aci-version` | Same |
| `GET /v1/aci/receipts/{id}` | Signed event log | Same + optional `solana.tee_proof_v2` event |
| `upstream.verified` | Aggregator must include it | Always present. Honest default: `result=failed`, `required=false` (`upstream_not_independently_verified`). Observed upstream ACI headers are recorded, not treated as verified |
| `GET /v1/aci/sessions/{id}` | Content-addressed upstream session | **Not implemented** |
| E2EE / TLS SPKI in keyset | Optional / required for plaintext HTTPS | Not published |
| Receipt storage | Memory + TTL | Memory + TTL (`RECEIPT_TTL_SECS`) |
| OpenAI `/v1/chat/completions` | Required | Required (plus completions + embeddings) |
| On-chain settlement | Out of scope (RedPill/EVM elsewhere) | **Solana-specific:** `StoreProofV2` → TeeProofV2 PDA. `attestation_report_hash` is the digest of quote + report_data + GPU verdict — not zeros when a quote exists |
| `_clawd` body field | — | CLAWD / Dark Pool compatibility |
| `/v1/attestation` | Legacy alias | Kept as alias of the ACI report, plus CLAWD fields (`signing_address`, `tdx_quote`, `gpu_claims`) |

## Tests

```bash
cargo test -p clawd-tee-gateway
sh scripts/smoke-aci.sh
```

Unit tests pin ACI keyset / `report_data` / receipt JCS vectors from
[private-ai-gateway spec/test-vectors.md](https://github.com/Dstack-TEE/private-ai-gateway/blob/main/spec/test-vectors.md)
and exercise receipt headers, nonce binding, and a mocked (no-keypair) anchor
path.
