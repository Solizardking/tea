# TEA

> public release of the CLAWD-TEE trust substrate  
> **T**EE attestation · **E**dge gateways · **A**nchor on Solana

Upstream private lineage: `clawd-tee`. This repo is the sanitized public cut.

---

```
 ▄████▄   ██▓    ▄▄▄       █     █░▓█████▄     ▄▄▄█████▓▓█████▓█████
▒██▀ ▀█  ▓██▒   ▒████▄    ▓█░ █ ░█░▒██▀ ██▌    ▓  ██▒ ▓▒▓█   ▀▓█   ▀
▒▓█    ▄ ▒██░   ▒██  ▀█▄  ▒█░ █ ░█ ░██   █▌    ▒ ▓██░ ▒░▒███  ▒███
▒▓▓▄ ▄██▒▒██░   ░██▄▄▄▄██ ░█░ █ ░█ ░▓█▄   ▌    ░ ▓██▓ ░ ▒▓█  ▄▒▓█  ▄
▒ ▓███▀ ░░██████▒▓█   ▓██▒░░██▒██▓ ░▒████▓       ▒██▒ ░ ░▒████▒▒████▒
░ ░▒ ▒  ░░ ▒░▓  ░▒▒   ▓▒█░░ ▓░▒ ▒   ▒▒▓  ▒       ▒ ░░   ░░ ▒░ ░░ ▒░ ░
  ░  ▒   ░ ░ ▒  ░ ▒   ▒▒ ░  ▒ ░ ░   ░ ▒  ▒         ░     ░ ░  ░░ ░  ░
░          ░ ░    ░   ▒     ░   ░   ░ ░  ░       ░         ░     ░
░ ░          ░  ░     ░  ░    ░       ░                    ░  ░  ░  ░
░                                   ░
```

# CLAWD-TEE

### self-hosted attestation · Solana settlement · replaceable gateways

```
╔══════════════════════════════════════════════════════════════════╗
║  STATUS   :: LIVE / DARKNET-READY                                ║
║  PLANE    :: control ≠ compute                                   ║
║  CHAIN    :: Solana SVM settles · TEEs compute · digests win     ║
║  VIBE     :: neon under rain · proofs over myths · digests > PR  ║
╚══════════════════════════════════════════════════════════════════╝
```

> **Solana SVM settles; TEEs compute; digests only — never put GPU on-chain.**  
> Stamp the route when you can. Ethereum / Sepolia / Automata are not production defaults (`--legacy-evm` only).  
> Policy: [`PROVIDER.md`](./PROVIDER.md).

This monorepo is the **trust substrate** for confidential AI on Solana: Pinocchio on-chain proof storage, a TEE-bound inference gateway, multi-provider attestation verifiers, OP-TEE / TrustZone paths, TypeScript clients, and the long-form architecture that names the whole system.

Inspired by the [RedPill verifier lineage](./redpill-verifier) — rewritten for **self-hosted SVM anchoring**, not someone else’s black-box.

---

## ⚡ Signal / Noise

| You want… | Go here |
| --- | --- |
| Run inference + stamp proofs | [`clawd-tee-gateway/`](./clawd-tee-gateway) |
| Gasless $CLAWD x402 (Kora relayer) | [`clawd-kora/`](./clawd-kora) · proxy: [`x402-proxy-template/`](./x402-proxy-template) · **public animated**: [`docs/gasless-aci.html`](./docs/gasless-aci.html) |
| Verify hardware (Phala / NearAI / Chutes / Tinfoil) | [`redpill-verifier/`](./redpill-verifier) · skill: [`verify-tee`](./redpill-verifier/.claude/skills/verify-tee/SKILL.md) |
| One package: verify **and** anchor | [`clawd-verifier/`](./clawd-verifier) |
| On-chain program (V1 + V2 PDAs) | [`program/`](./program) |
| TypeScript SDK + CLI | [`clients/typescript/`](./clients/typescript) |
| Arm TrustZone signer | [`optee/`](./optee) |
| Nested package mirror (publish / monorepo handoff) | [`solana-tee/`](./solana-tee) |
| Why this exists (soul + essays) | [`content/`](./content) |
| Historical Ethereum → Solana port notes | [`svm.md`](./svm.md) |
| Run as a RedPill-compatible provider (ACI + SVM settlement) | [`PROVIDER.md`](./PROVIDER.md) |
| Don’t leak keys before open-source | [`PUBLISHING.md`](./PUBLISHING.md) · [`SECURITY.md`](./SECURITY.md) |

---

## 🕳 Soul of the system

Character harness lives under **`content/soul/`**. Design essays under **`content/articles/`**. When marketing slogans conflict with long form — **the articles win**.

### Soul load order

1. Parent constitution / six-laws (if present in agent context)
2. [`content/soul/SOUL.md`](./content/soul/SOUL.md)
3. Both essays under [`content/articles/`](./content/articles/)
4. This `README.md`
5. Runtime surfaces (gateway, verifiers, Solana)

From the soul file, the web is a **Relay shell** — not branding theater. Core health and public models do not require a token cult. Holder gates unlock premium features; they do not rewrite the constitution.

```
┌─────────────── CLAWD SURFACES ───────────────┐
│  GRID    · compute mesh / provider ops        │
│  FORGE   · training · adapters · datasets     │
│  RELAY   · chat · models · replaceable BFF    │
│  REGISTRY· content-addressed model identity   │
│  VERIFY  · TEE · RedPill · task-specific truth│
└───────────────────────────────────────────────┘
```

| Principle ([Brave New World](./content/articles/brave-new-world-blockchain-ai.md)) | How this repo lives it |
| --- | --- |
| Transparent provenance | TeeProofV2 PDAs · quote / payload hashes · audit-friendly RPC lookup |
| Decentralized training coordination | Solana as settlement; providers keep the silicon |
| Consensus Learning (predictions > raw minds) | Privacy-aware flows; never demand private weights |
| On-chain reinforcement loop (ORL) | Observe → route → stamp → reward signals when ZK / TEE proofs exist |
| Solana-native fitness | sub-cent fees, PDAs, SAS credentials — not GPUs in the SVM |

| Surface ([Clawd SVM design](./content/articles/clawd-solana-svm-design.md)) | Repo expression |
| --- | --- |
| **Grid** | Provider-shaped ops, gateway mesh, OP-TEE / TDX hardware paths |
| **Forge** | Essays + design for adapters / federated rounds (not weights on-chain) |
| **Relay** | `clawd-tee-gateway` — OpenAI-compatible proxy, replaceable |
| **Registry** | digests, model hashes, IDL, content-addressed artifacts |
| **Verify** | `redpill-verifier` · `clawd-verifier` · `program` · on-chain levels 0–5 |

**Non-goals (carved in dark glass):** stuffing model weights onto Solana · claiming privacy solely because a job is coordinated on-chain · one static score for every workload · gating `/health` behind mythology.

---

## 🗺 Full monorepo map

```
clawd-tee/                          ◄── you are here (workspace root)
│
├── 📜 README.md                    this file — map + ritual
├── 📜 svm.md                       historical V1 handoff (Eth → Solana)
├── 📜 PUBLISHING.md                pre-publish secret / history guards
├── 📜 SECURITY.md                  threat model & hard rules
├── 📜 PROVIDER.md                  RedPill / ACI compatibility + Fly ops
├── 📜 render.yaml                  deploy surface notes
├── ⚙  Cargo.toml                   workspace: program + clawd-tee-gateway
├── ⚙  Cargo.lock
│
├── 🧬 content/                     ◄── doctrine & long-form (soul sources)
│   ├── articles/
│   │   ├── brave-new-world-blockchain-ai.md
│   │   │     themes: provenance · federated learning · Consensus Learning · ORL
│   │   └── clawd-solana-svm-design.md
│   │         themes: Grid / Forge / Relay / Registry / Verify · control ≠ compute
│   └── soul/
│       └── SOUL.md                 character harness for Relay UI + agents
│
├── ⛓ program/                      Pinocchio on-chain (zero-std Rust)
│   └── src/
│       ├── entrypoint.rs           disc 0 InitCounter · 1 StoreProof · 2 StoreProofV2
│       ├── error.rs
│       ├── state/
│       │   ├── tee_proof.rs        TeeProof · TeeProofV2 · VerifierRegistry
│       │   └── proof_counter.rs    ProofCounter + ProofCounterDay shards
│       └── processor/
│           ├── store_proof.rs      V1
│           └── store_proof_v2.rs   V2 flexible secp + day counters
│
├── 📡 idl/
│   └── solana_redpill_verifier.json
│
├── ⛽ clawd-kora/                   Kora gasless relayer + x402 facilitator
│   ├── kora.toml · signers.toml    $CLAWD mint allowlist (8cHz…pump)
│   ├── scripts/start-kora.sh
│   └── facilitator/                GET /supported · POST /verify · POST /settle
│
├── 💳 x402-proxy-template/          HTTP 402 gate in front of the TEE gateway
│                                   Solana/Kora rail (not Base Sepolia)
│
├── 🖥 clawd-tee-gateway/            TEE inference proxy (Rust / axum)
│   ├── src/
│   │   ├── main.rs                 AppState · router · SAS init
│   │   ├── config.rs               env: Solana · NVIDIA · SAS
│   │   ├── signing.rs              TEE secp256k1 (k256) · signedPayloadHash
│   │   ├── attestation.rs          Intel TDX ioctl + NVIDIA NRAS
│   │   ├── proxy.rs                OpenAI-compat routes + wallet tracking
│   │   ├── solana.rs               StoreProofV2 anchor
│   │   ├── sas.rs                  Solana Attestation Service credentials
│   │   ├── types.rs · error.rs
│   ├── Dockerfile · docker-compose.yml · .env.example
│   └── .env.example
│
├── 🛡 optee/                        CLAWD OP-TEE (Arm TrustZone)
│   ├── include/clawd_ta.h          host ↔ TA ABI
│   ├── host/                       GlobalPlatform Client API bridge
│   ├── ta/                         trusted app signer boundary
│   ├── PROTOCOL.md                 OP-TEE → StoreProofV2 mapping
│   └── POSITIONING.md
│
├── 📦 clients/typescript/          @redpill-ai/solana-verifier
│   └── src/
│       ├── accounts.ts · instructions.ts · pdas.ts · types.ts
│       ├── verify.ts               storeProofV2Onchain / lookupProofV2
│       ├── cli.ts                  init-counter · store-* · lookup · day-counter
│       └── index.ts
│
├── 👁 clawd-verifier/               unified @clawd/tee-verifier
│   ├── js/src/                     core verifiers + solana client + anchor
│   ├── contracts/evm/              ClawdProofStore.sol
│   ├── contracts/solana/           IDL mirror
│   └── docker-compose.yml
│
├── 💊 redpill-verifier/            multi-provider TEE engine (origin stack)
│   ├── attestation_verifier.py · onchain_proof.py · signature_verifier.py
│   ├── js/                         @redpill-ai/verifier
│   ├── contracts/                  RedpillProofStore (EVM)
│   └── .claude/skills/verify-tee/  agent skill: light + deep modes
│
├── 🪞 solana-tee/                  nested package mirror (same stack, portable)
│   ├── program/ · clients/ · clawd-tee-gateway/ · optee/ · idl/ · scripts/
│   ├── README.md · svm.md · render.yaml · PUBLISHING.md · SECURITY.md
│   └── .github/                    CI / dependabot for the nested package
│
├── 🔧 scripts/
│   ├── security-scan.sh
│   ├── check-publish-history.sh
│   ├── test-security-guards.sh
│   └── verify-imported-trees.sh    structural import / layout gate
│
└── .github/                        root CI surface
```

```
        ╔════════ INFERENCE ════════╗
        ║  client / agent / UI      ║
        ╚═══════════╤═══════════════╝
                    │  OpenAI-shaped HTTP
                    ▼
        ┌───────────────────────────┐
        │   CLAWD TEE GATEWAY       │  TDX · H100 CC · OP-TEE option
        │   sign · attest · proxy   │
        └───────────┬───────────────┘
                    │  StoreProofV2 + secp256k1
                    ▼
        ┌───────────────────────────┐
        │   SOLANA SVM PROGRAM      │  TeeProofV2 PDA · day counters
        │   digests permanent       │
        └───────────┬───────────────┘
                    │  threshold reached
                    ▼
        ┌───────────────────────────┐
        │   SAS Token-2022 NFT      │  non-transferable credential
        └───────────────────────────┘

   VERIFY plane (off-chain): redpill-verifier · clawd-verifier · Phala / NRAS / DCAP
   DOCTRINE plane:           content/articles · content/soul · svm.md (history)
```

---

## 📚 Doctrine index (`content/`)

| Path | Role |
| --- | --- |
| [`content/soul/SOUL.md`](./content/soul/SOUL.md) | Character harness: how Relay behaves; load order; non-goals |
| [`content/articles/brave-new-world-blockchain-ai.md`](./content/articles/brave-new-world-blockchain-ai.md) | *Brave New World: Why Blockchain + AI?* — provenance, decentralized training, Consensus Learning, **ORL**, Solana as coordination |
| [`content/articles/clawd-solana-svm-design.md`](./content/articles/clawd-solana-svm-design.md) | *Clawd: Decentralized AI and Compute on Solana SVM* — Grid / Forge / Relay / Registry / Verify protocol draft |
| [`svm.md`](./svm.md) | **Historical V1 handoff** — Ethereum `RedpillProofStore` → Solana PDAs; not the V2 source of truth (this README is) |

> *I'm the Relay dashboard. Show the essays. Stamp the route when you can. Digests beat myths.*  
> — SOUL.md

---

## ⛓ On-chain program

### Instructions

| Disc | Name | Description |
| --- | --- | --- |
| `0` | `InitCounter` | One-time setup: global proof counter PDA |
| `1` | `StoreProof` | V1 — verified TEE proof; optional secp256k1 |
| `2` | `StoreProofV2` | V2 — flexible secp ix index, payload binding, sharded day counter |

### Accounts

#### `TeeProof` PDA (V1)

Seeds: `["tee_proof", quote_hash[0..32]]`

| Field | Type | Description |
| --- | --- | --- |
| `quote_hash` | `[u8;32]` | SHA256 of raw TDX quote bytes |
| `signing_address` | `[u8;20]` | Ethereum-style TEE signing address |
| `is_valid` | `bool` | DCAP verification passed off-chain |
| `timestamp` | `i64` | Unix time stored |
| `slot` | `u64` | Solana slot |
| `submitter` | `Pubkey` | Who submitted |

#### `TeeProofV2` PDA (V2, 372 bytes)

Seeds: `["tee_proof_v2", signed_payload_hash[0..32]]`

| Field | Type | Description |
| --- | --- | --- |
| `version` | `u8` | schema (`2`) |
| `verification_level` | `u8` | 0 StoredOnly · 1 TeeSigOnchain · 2 DcapOffchain · 3 MultiVerifier · 4 DcapOnchain · 5 ZkAuthorized |
| `status` | `u8` | 0 Active · 1 Consumed · 2 Revoked · 3 Expired |
| `quote_hash` | `[u8;32]` | SHA256(TDX quote) |
| `signed_payload_hash` | `[u8;32]` | `SHA256("CLAWD_REDPILL_V2:" ‖ req ‖ res ‖ model ‖ nonce)` |
| `request_hash` / `response_hash` / `model_hash` | `[u8;32]` | inference binding |
| `attestation_report_hash` | `[u8;32]` | full report digest |
| `compose_hash` | `[u8;32]` | Docker Compose digest |
| `signing_address` | `[u8;20]` | TEE eth-style address |
| `verifier_set_hash` | `[u8;32]` | authorized verifiers |
| `policy_version` | `u32` | policy at issuance |
| `nonce` | `[u8;32]` | per-proof entropy |
| `expires_at` | `i64` | `0` = no expiry |
| `timestamp` / `slot` / `submitter` | … | chain metadata |

#### `ProofCounterDay` PDA

Seeds: `["proof_counter_day", day_tag[0..8]]` · `day_tag = b"YYYYMMDD"`  
Auto-created first proof of each UTC day — no global write-lock choke point.

---

## 🔮 V2 verification flow

```
 Inference Request
        │
        ▼
 ┌──────────────────────────────────────────────────────────┐
 │  CLAWD TEE Gateway  ·  Intel TDX + NVIDIA H100 CC        │
 │                                                          │
 │  1. Forward → upstream AI provider                       │
 │  2. sha256(request)  → request_hash                      │
 │  3. sha256(response) → response_hash                     │
 │  4. sha256(model)    → model_hash                        │
 │  5. nonce = random 32B                                   │
 │  6. signed_payload_hash = SHA256(                        │
 │       "CLAWD_REDPILL_V2:" ‖ req ‖ res ‖ model ‖ nonce)   │
 │  7. Sign inside sealed / TEE-backed boundary             │
 └──────────────────────────────────────────────────────────┘
        │
        ▼
 Attestation (parallel):
   TDX  → TDX_CMD_GET_QUOTE on /dev/tdx_guest
   GPU  → NVIDIA NRAS JWT (genuine H100/H200 CC)
        │
        ▼
 Solana anchor (StoreProofV2):
   secp256k1 native ix  (level ≥ 1)
   StoreProofV2 → TeeProofV2 PDA  → permanent record
   ProofCounterDay auto-bump
        │
        ▼
 Response header `_clawd` proof block:
   { request_hash, response_hash, model_hash,
     signed_payload_hash, signing_address, signature,
     verification_level, solana_proof_address, solana_tx_sig }
        │
        ▼
 SAS credential (count ≥ SAS_PROOF_THRESHOLD):
   x-clawd-wallet: <base58>
   → Token-2022 non-transferable NFT
   → { proof_count, tee_signing_address, last_proof_hash }
```

---

## 🖥 CLAWD TEE Gateway

Rust/axum HTTP service: OpenAI-compatible proxy · proof signing · optional TDX + NRAS · optional Solana anchor · optional SAS credentials.

`optee/` is the Arm TrustZone twin for the same `signed_payload_hash` shape — TDX, OP-TEE, and future providers share one `StoreProofV2` account model.

RedPill / ACI provider ops, header contract, and what still needs real TDX
hardware live in [`PROVIDER.md`](./PROVIDER.md).

### Endpoints

| Method | Path | Description |
| --- | --- | --- |
| `GET` | `/health` | liveness · signing address · ACI identity · SAS status |
| `GET` | `/v1/aci/attestation?nonce=` | ACI report bound to a 64-hex nonce |
| `GET` | `/v1/attestation` | alias of the ACI report (+ CLAWD fields) |
| `GET` | `/v1/aci/receipts/{id}` | signed receipt (`rcpt-…` or chat id); memory+TTL |
| `POST` | `/v1/chat/completions` | chat (proxied + attested + receipt headers) |
| `POST` | `/v1/completions` | completions (proxied + attested + receipt headers) |
| `POST` | `/v1/embeddings` | embeddings (proxied + attested + receipt headers) |
| `GET` | `/v1/proof/:payload_hash` | TeeProofV2 by signedPayloadHash |
| `POST` | `/v1/credential/init` | admin: SAS credential + schema |
| `POST` | `/v1/credential/issue` | admin: issue to wallet |
| `GET` | `/v1/credential/:wallet` | attestation status + expiry |

Inference responses add `x-receipt-id`, `x-aci-identity`,
`x-aci-keyset-digest`, and `x-aci-version`. Every inference path collects TDX
evidence and attempts `StoreProofV2` when a Solana keypair is configured —
skips are logged, never silent. `attestation_report_hash` is non-zero when a
quote exists.

Pass `x-clawd-wallet: <base58>` on inference to track proofs and auto-issue SAS.

Admin routes need `ADMIN_TOKEN` via `x-clawd-admin-token` or `Authorization: Bearer …`. No token configured → those routes are dead.

### Quick start (Docker)

```bash
cd clawd-tee-gateway
cp .env.example .env
# UPSTREAM_API_KEY + ADMIN_TOKEN required
# SOLANA_KEYPAIR_PATH only if anchoring on-chain

docker compose up --build
```

### Quick start (local)

```bash
cd clawd-tee-gateway
UPSTREAM_URL=https://api.openai.com \
UPSTREAM_API_KEY=<provider-api-key> \
ADMIN_TOKEN=<long-random-admin-token> \
SOLANA_KEYPAIR_PATH=~/.config/solana/id.json \
REDPILL_PROGRAM_ID=BnoUSPTE88ebzLb74RAnTogddDMzyBPtkmKxiRoRJG4L \
SAS_PROOF_THRESHOLD=5 \
cargo run
```

### Hosted (Railway)

Public BASE: `https://clawd-tee-gateway-production.up.railway.app`

`GET /health`, ACI attestation, chat/completions, receipts, and proof lookup
speak the same protocol as local. Operator path: [`PROVIDER.md`](./PROVIDER.md).
[`render.yaml`](./render.yaml) remains an alternate Blueprint.

```bash
npx clawd-verifier health --gateway-url https://clawd-tee-gateway-production.up.railway.app
```

### Key environment variables

| Variable | Default | Description |
| --- | --- | --- |
| `PORT` | `8080` | listen port |
| `BIND_ADDR` | `0.0.0.0` | bind |
| `UPSTREAM_URL` | — | upstream AI base URL |
| `UPSTREAM_API_KEY` | — | upstream key (unless client auth enabled) |
| `ALLOW_CLIENT_AUTHORIZATION` | `false` | accept caller `Authorization` |
| `ALLOW_INSECURE_UPSTREAM` | `false` | allow `http://` upstream (dev only) |
| `SOLANA_RPC_URL` | devnet | Solana RPC |
| `REDPILL_PROGRAM_ID` | `BnoUSPTE88…` | deployed program |
| `SOLANA_KEYPAIR_PATH` | — | fee payer; omit = no anchor |
| `SIGNING_KEY_PEM_PATH` | — | persist TEE signing key |
| `NVIDIA_API_KEY` | — | NRAS (optional) |
| `PROOF_TTL_SECS` | `86400` | expiry (`0` = none) |
| `VERIFICATION_LEVEL` | `0` | 0 StoredOnly · 1 TeeSigOnchain |
| `SAS_PROOF_THRESHOLD` | `0` | proofs before SAS (`0` = off) |
| `ADMIN_TOKEN` | — | credential admin routes |
| `CORS_ALLOWED_ORIGIN` | — | concrete origin; empty = CORS off |
| `MAX_REQUEST_BYTES` | `1048576` | body cap |
| `RECEIPT_TTL_SECS` | `3600` | in-memory ACI receipt lifetime |
| `RECEIPT_MAX_ENTRIES` | `10000` | receipt store cap |
| `WORKLOAD_SUBJECT` | `clawd-tee-gateway` | ACI keyset subject (not a trust anchor) |
| `SOURCE_REPO_URL` | this GitHub repo | attestation provenance label |
| `SOURCE_REPO_COMMIT` | — | optional commit pin |
| `SAS_CREDENTIAL_NAME` | `CLAWD-TEE-GATEWAY` | SAS credential |
| `SAS_SCHEMA_NAME` | `CLAWD-TEE-PROOF-V1` | SAS schema |
| `SAS_TOKEN_NAME` / `SYMBOL` / `URI` | … | NFT metadata |
| `SAS_EXPIRY_SECS` | `2592000` | 30d default |

---

## 🪪 Solana Attestation Service (SAS)

Integrates [SAS](https://github.com/solana-foundation/solana-attestation-service) (`22zoJMtdu4tQc2PzL74ZUT7FrwgB1Udec8DdW4yw4BdG`).

Each attestation = **Token-2022 non-transferable NFT**:

```
proof_count          — verified TEE inferences for this wallet
tee_signing_address  — gateway TEE eth-style address
last_proof_hash      — latest signedPayloadHash (hex)
```

```
Credential   ["credential",   authority,       credential_name]
Schema       ["schema",       credential_pda,  schema_name, schema_version]
Attestation  ["attestation",  credential_pda,  schema_pda,  wallet_pubkey]
Schema mint  ["schemaMint",   schema_pda]
Att. mint    ["attestationMint", attestation_pda]
```

1. Client sends `x-clawd-wallet` on every inference.  
2. After each successful anchor, in-memory counter ++.  
3. At `count >= SAS_PROOF_THRESHOLD` (and multiple of threshold): re-issue attestation via `CreateTokenizedAttestationBuilder`.  
4. Wallet holds explorer-visible on-chain credential.

---

## 📦 TypeScript client (V2)

```typescript
import { Keypair } from '@solana/web3.js'
import {
  storeProofV2Onchain,
  lookupProofV2,
  lookupDayCounter,
  computeSignedPayloadHash,
  VerificationLevel,
} from '@redpill-ai/solana-verifier'

const signer = Keypair.fromSecretKey(/* Uint8Array from a non-repo secret */)

const signedPayloadHash = await computeSignedPayloadHash(mySignedMessage)

const result = await storeProofV2Onchain(
  { rpcUrl: 'https://api.devnet.solana.com', programId: 'YOUR_PROGRAM_ID' },
  signer,
  {
    signedPayloadHash,
    signingAddress: ethAddress,
    requestHash,
    responseHash,
    modelHash,
    verificationLevel: VerificationLevel.TeeSigOnchain,
    signature,
  },
)

console.log(result.txSig, result.proofAddress)

const proof = await lookupProofV2(
  { rpcUrl: '...', programId: '...' },
  signedPayloadHash,
)

const counter = await lookupDayCounter(network, '20260619')
```

### CLI

```bash
cd clients/typescript && npm install && npm run build

node dist/cli.js store-model --model deepseek/deepseek-v4-flash
node dist/cli.js lookup-v2 --payload-hash <64-hex-chars>
node dist/cli.js day-counter --day 20260619
node dist/cli.js hash-message --message "CLAWD_REDPILL_V2:..."
node dist/cli.js lookup --quote-hash <64-hex-chars>   # V1
```

### Unified verifier package

```bash
cd clawd-verifier/js && npm install && npm run build
# see clawd-verifier/README.md — verifyAndAnchor()
```

### RedPill origin engine

```bash
cd redpill-verifier/js && npm install
# or: Python entrypoints + contracts/ for EVM
# agent skill: redpill-verifier/.claude/skills/verify-tee/SKILL.md
```

---

## 🏗 Build

```bash
# On-chain program (workspace root)
cargo build-sbf
solana program deploy target/deploy/solana_redpill_verifier.so --url devnet

# TypeScript client
cd clients/typescript && npm install && npm run build

# Gateway
cd clawd-tee-gateway && cargo build --release

# Nested package mirror (same stack, portable tree)
cd solana-tee && cargo metadata --no-deps
```

Before publishing, run [`PUBLISHING.md`](./PUBLISHING.md) checks:

```bash
sh scripts/security-scan.sh
sh scripts/check-publish-history.sh
sh scripts/test-security-guards.sh
sh scripts/verify-imported-trees.sh
```

Removing a secret from the index is **not** enough if it still lives in Git history.

---

## ⚖ Comparison: V1 vs V2 vs Ethereum origin

| | Ethereum (RedPill) | Solana V1 | Solana V2 (CLAWD) |
| --- | --- | --- | --- |
| **Key identifier** | `keccak256(quote)` | `sha256(quote)` | `SHA256("CLAWD_REDPILL_V2:" ‖ …)` |
| **Message signed by TEE** | `signature.text` | `quote_hash` | `signed_payload_hash` (32B) |
| **On-chain sig verify** | `ecrecover` | secp fixed ix 0 | secp flexible ix index |
| **Proof counter** | mapping length | global PDA | sharded `ProofCounterDay` |
| **Payload binding** | event hash | quote only | req ‖ res ‖ model ‖ nonce |
| **Credentials** | none | none | SAS Token-2022 NFT |
| **Self-hosted gateway** | no | no | yes |
| **Storage cost** | ETH gas | ~0.002 SOL | ~0.003 SOL (372B) |
| **Lookup** | contract read | `getAccountInfo` | `getAccountInfo` |

Historical port detail → [`svm.md`](./svm.md). Live architecture → this file + [`content/articles/clawd-solana-svm-design.md`](./content/articles/clawd-solana-svm-design.md).

---

## 🌑 Closing ritual

```
  digests > myths
  control plane ≠ compute plane
  gateways are replaceable
  show the essays
  stamp the route when you can
```

Read the soul. Deploy the program. Run the gateway. Verify the silicon.  
The rain is optional. The hashes are not.

```
░░ CLAWD-TEE · SOLANA · TEE · PROOFS · FOREVER ░░
```
