# CLAWD Verifier

One package for the whole trust chain: verify that AI responses come from
genuine TEE hardware, then anchor the proof on Solana.

**Solana SVM settles; TEEs compute; digests only — never put GPU on-chain.**
Default cluster is `devnet` (`https://api.devnet.solana.com`). Ethereum /
Sepolia / Holesky / Automata require `--legacy-evm`. See [`../PROVIDER.md`](../PROVIDER.md).

This is the unified successor to two codebases:

- [`../redpill-verifier`](../redpill-verifier) — multi-provider TEE
  verification engine (Phala / NearAI / Chutes / Tinfoil, light + deep modes,
  Automata DCAP, Intel Trust Authority, EVM proof store)
- [`../solana-redpill-verifier`](../solana-redpill-verifier) — Solana verifier
  program (TeeProof V1/V2 PDAs, native secp256k1 verification), TypeScript
  client, and the `clawd-tee-gateway` Rust service

Both engines now live in a single npm package with one API and one CLI.

```typescript
import { verifyAndAnchor } from '@clawd/tee-verifier'

const result = await verifyAndAnchor({
  model: 'phala/gpt-oss-120b',
  solana: { rpcUrl, programId, signer: keypair },
})
result.verified              // true — TDX + report_data + GPU all checked
result.solana?.txSig         // Solana tx anchoring the proof
result.solana?.proofAddress  // PDA anyone can look up, forever
```

## Layout

```
js/                      @clawd/tee-verifier — the unified npm package
  src/
    anchor.ts            verifyAndAnchor() — verify once, anchor everywhere
    core/                multi-provider TEE verification engine
      verifiers/         cloud-api, dstack (deep), onchain (Automata), tinfoil,
                         chutes, intel-ita
    solana/              Solana client — PDAs, instructions, V1/V2 storage,
                         secp256k1 precompile, day counters
  bin/cli.ts             clawd-verifier CLI
contracts/
  evm/                   ClawdProofStore.sol (+ ABI/bin) — Automata DCAP
                         verify-and-store
  solana/                program IDL (source: ../solana-redpill-verifier/program)
docker-compose.yml       dstack-verifier (deep mode) + clawd-tee-gateway
```

## Install & build

```bash
cd js && npm install && npm run build
```

## CLI

```bash
# TEE attestation only (no API key)
npx clawd-verifier attest --model phala/gpt-oss-120b

# Deep mode — trust only Intel silicon (requires docker compose up -d)
npx clawd-verifier attest --model phala/qwen-2.5-7b-instruct --deep

# Verify an existing chat response (signature + attestation)
npx clawd-verifier verify chatcmpl-xxx --api-key sk-xxx --model phala/gpt-oss-120b

# Verify + anchor on Solana (V1 quote-hash proof)
npx clawd-verifier anchor --keypair ~/payer.json \
  --model phala/gpt-oss-120b --rpc-url https://api.devnet.solana.com

# Verify + anchor a specific chat response (V2 proof with request/response hashes)
npx clawd-verifier anchor --keypair ~/payer.json \
  --request-id chatcmpl-xxx --api-key sk-xxx --model phala/gpt-oss-120b

# Look up anchored proofs on Solana (free)
npx clawd-verifier lookup --quote-hash <sha256-of-quote> --rpc-url https://api.devnet.solana.com
npx clawd-verifier lookup --payload-hash <sha256-of-signed-text> --rpc-url https://api.devnet.solana.com

# Deprecated EVM path (requires --legacy-evm)
npx clawd-verifier lookup-evm --legacy-evm --quote-hash 0x... --proof-store 0x... --network sepolia

# Gateway health (local compose / cargo run)
npx clawd-verifier health --gateway-url http://127.0.0.1:8081

# Hosted Railway gateway
npx clawd-verifier health --gateway-url https://clawd-tee-gateway-production.up.railway.app
```

Defaults point at Solana **devnet** (`https://api.devnet.solana.com`), program
`HvXe5RpCcduVYJWDyavmNKde4uEgUFEiYz99JYQT4DkP`. Override `--rpc-url` for
`mainnet-beta` or a local validator (`http://127.0.0.1:8899`).

## API

Three layers, one import:

```typescript
import {
  verifyAndAnchor,        // unified pipeline (recommended)
  verify, verifyModel,    // core verification (same API as @redpill-ai/verifier)
  tee,                    // full core engine, namespaced
  solana,                 // full Solana client, namespaced
} from '@clawd/tee-verifier'

// Namespaced engines keep their complete APIs:
const proof = await solana.lookupProofV2({ rpcUrl, programId }, payloadHash)
const quote = tee.parseTdxQuote(quoteHex)
```

### `verifyAndAnchor(options)`

1. **Verify** — provider auto-detection, TDX quote (Phala cloud API), report
   data binding (signing address + nonce), GPU attestation (NVIDIA NRAS),
   compose manifest, Sigstore provenance, optional dstack deep replay, optional
   Automata on-chain DCAP, optional Intel Trust Authority.
2. **Signature** (with `chat: { requestId, apiKey }`) — fetches the TEE's
   ECDSA signature over `sha256(request):sha256(response)` and checks it via
   ecrecover.
3. **Anchor**:
   - `solana: { rpcUrl, programId, signer }` — with a chat binding, stores a
     **TeeProofV2** record (signed-payload hash, request/response/model/quote
     hashes); attestation-only runs store a **TeeProof V1** record (quote hash
     + verdict).
   - `evm: { privateKey, proofStore, network, legacyEvm: true }` — **deprecated**.
     Calls `ClawdProofStore.verifyAndStore` on Ethereum / Automata. Throws
     without `legacyEvm: true`.

## Verification levels (Solana V2 records)

| Level | Meaning |
|---|---|
| 0 `StoredOnly` | hashes anchored, forensic link only |
| 1 `TeeSigOnchain` | secp256k1 signature verified on-chain via native precompile |
| 2 `DcapOffchain` | DCAP quote verified off-chain by a registered verifier |
| 3 `MultiVerifier` | quorum of verifiers |
| 4 `DcapOnchain` | full DCAP verification on-chain |
| 5 `ZkAuthorized` | ZK proof of verification |

RedPill chat signatures anchor at level 0 (the TEE signs prefixed text, not the
32-byte payload hash). The `clawd-tee-gateway` signs payload hashes directly
and anchors at level 1.

## Trust model

**Light mode trusts**: Phala's verification API, NVIDIA NRAS.
**Deep mode trusts**: only Intel/AMD silicon (QEMU replays boot measurements).
**Solana anchoring (default)**: TeeProof / TeeProofV2 PDAs; digests only.
**EVM anchoring**: legacy Automata DCAP; requires `--legacy-evm`.

## Related services

- `clawd-tee-gateway` (Rust, port 8081 local / Railway hosted) — proxies
  inference, signs inside the TEE, anchors level-1 V2 proofs autonomously.
  Source: [`../clawd-tee-gateway`](../clawd-tee-gateway).
  Hosted: `https://clawd-tee-gateway-production.up.railway.app`
- `dstack-verifier` (Docker, port 8080) — deep-mode measurement replay.

## License

MIT
