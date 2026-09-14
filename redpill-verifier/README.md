# RedPill Verifier

Verify that AI responses come from genuine TEE hardware. One npm package, two verification depths, four providers.

**Solana SVM settles; TEEs compute; digests only — never put GPU on-chain.** On-chain store/lookup defaults to Solana TeeProof via [`clawd-verifier`](../clawd-verifier). Ethereum / Sepolia / Holesky / Automata require `--legacy-evm`. See [`../PROVIDER.md`](../PROVIDER.md).

```typescript
import { verify } from '@redpill-ai/verifier'

const proof = await verify(response.id, { model: 'phala/gpt-oss-120b', apiKey: 'sk-xxx' })
// proof.verified === true
```

## Install

```bash
cd js && npm install
```

## Verify your chat response

Use RedPill as an OpenAI drop-in, then verify the response:

```typescript
import OpenAI from 'openai'
import { verify } from '@redpill-ai/verifier'

const openai = new OpenAI({ baseURL: 'https://api.redpill.ai/v1', apiKey: 'sk-xxx' })
const response = await openai.chat.completions.create({
  model: 'phala/gpt-oss-120b',
  messages: [{ role: 'user', content: 'What is confidential computing?' }],
})

const proof = await verify(response.id, { model: 'phala/gpt-oss-120b', apiKey: 'sk-xxx' })
console.log(proof.verified)                     // true
console.log(proof.signature.valid)              // true — ECDSA ecrecover
console.log(proof.provider)                     // "near-ai"
console.log(proof.hardware)                     // ["INTEL_TDX", "NVIDIA_CC"]
```

## Verify attestation (no API key)

```typescript
import { verifyModel } from '@redpill-ai/verifier'

const result = await verifyModel({ model: 'phala/gpt-oss-120b' })
// result.light.tdx.verified === true
// result.light.gpu.verdict === "true"
// result.light.reportData.bindsAddress === true
```

## Two verification modes

### Light mode (default) — no Docker

Uses cloud APIs: Phala TDX verifier + NVIDIA NRAS. On-chain settlement is Solana TeeProof (not Automata).

```typescript
const result = await verifyModel({ model: 'phala/gpt-oss-120b' })
```

### Deep mode — trust only Intel silicon

Uses dstack-verifier (Rust + QEMU) to independently replay boot measurements. Requires Docker.

```bash
docker compose up -d   # start dstack-verifier
```

```typescript
const result = await verifyModel({ model: 'phala/qwen-2.5-7b-instruct', deep: true })
// result.deep.components[0] = { name: "model", result: { isValid: true, quoteVerified: true, ... } }
// result.deep.components[1] = { name: "kms", result: { isValid: true, ... } }
// result.deep.components[2] = { name: "gateway", result: { isValid: true, ... } }
```

## Four providers

RedPill routes to different TEE providers. The verifier auto-detects and applies provider-specific checks.

| Provider | Models | Hardware | Provider-specific checks |
|---|---|---|---|
| **Phala** | gpt-oss-20b, qwen-2.5-7b, gemma-3-27b, ... | TDX + NVIDIA CC | Compose manifest, deep mode (model+KMS+gateway) |
| **NearAI** | gpt-oss-120b, deepseek-chat-v3.1, glm-4.7, ... | TDX + NVIDIA CC | Gateway+model attestation, deep mode |
| **Chutes** | deepseek-v3.2, kimi-k2.5, ... | TDX + NVIDIA CC | Anti-tamper binding (nonce+e2e_pubkey), debug mode check |
| **Tinfoil** | llama-3.3-70b, kimi-k2-thinking, deepseek-r1, ... | TDX or SEV-SNP | Hardware policy (MR_SEAM, XFAM), Sigstore golden values, hw profiles |

## On-chain proof storage (Solana SVM)

Default settlement is Solana `devnet` / `mainnet-beta` via clawd-verifier TeeProof / TeeProofV2. Digests only — never put GPU on-chain.

```typescript
import { verifyAndAnchor, solana } from '@clawd/tee-verifier'

const result = await verifyAndAnchor({
  model: 'phala/gpt-oss-120b',
  solana: { rpcUrl: 'https://api.devnet.solana.com', programId, signer: keypair },
})
// result.solana?.txSig — TeeProof / TeeProofV2 PDA on Solana

const record = await solana.lookupProof({ rpcUrl: 'https://api.devnet.solana.com', programId }, quoteHash)
```

Ethereum / Automata DCAP is legacy. `storeProof` / `lookupProof` / `verifyOnchain` throw unless you pass `{ legacyEvm: true }` and an explicit EVM network.

## Intel Trust Authority (optional)

Secondary verification via Intel's own attestation service. Works with all providers.

```typescript
const result = await verifyModel({
  model: 'phala/gpt-oss-120b',
  itaApiKey: 'your-intel-trust-authority-key',
})
// result.ita.appraised === true
```

## CLI

```bash
# Verify existing chat response
npx redpill-verifier verify chatcmpl-xxx --api-key sk-xxx --model phala/gpt-oss-120b

# Attestation only (no API key)
npx redpill-verifier attestation --model phala/gpt-oss-120b

# Deep mode (requires Docker)
npx redpill-verifier attestation --model phala/qwen-2.5-7b-instruct --deep

# Light mode only (skip Docker even if available)
npx redpill-verifier attestation --model phala/gpt-oss-120b --light

# Store / look up proofs on Solana (recommended)
npx clawd-verifier anchor --keypair ~/.config/solana/id.json --rpc-url https://api.devnet.solana.com
npx clawd-verifier lookup --quote-hash <hex> --rpc-url https://api.devnet.solana.com

# Deprecated EVM store (requires --legacy-evm)
npx redpill-verifier store --legacy-evm --private-key 0x... --proof-store 0x... --network sepolia
```

## What gets verified

```
User Request
    |
    v
+-----------------------------------------------+
|  TEE Enclave (Intel TDX / AMD SEV-SNP)        |
|  + NVIDIA H100/H200 GPU (Confidential Computing)|
|                                               |
|  1. Receives request                          |
|  2. Runs inference on model                   |
|  3. SHA256(request) : SHA256(response)        |
|  4. Signs with Ethereum private key           |
|     (key NEVER leaves the enclave)            |
+-----------------------------------------------+
    |
    v
Verification chain:
  Signature  -> ecrecover -> proves signing_address signed these hashes
  TDX Quote  -> Phala API or dstack-verifier -> genuine TDX/SEV-SNP enclave
  Report Data -> signing_address + nonce bound to hardware
  GPU        -> NVIDIA NRAS -> genuine H100/H200 in confidential mode
  Compose    -> mr_config match -> exact code + model weights verified
  Sigstore   -> container build provenance verified
  Tinfoil    -> hardware policy + Sigstore golden values
  Chutes     -> anti-tamper binding (nonce + e2e_pubkey)
  On-Chain   -> Solana TeeProof / TeeProofV2 (clawd-verifier) — default
  ITA        -> Intel Trust Authority -> independent appraisal
  Proof Store -> Solana PDA anyone can query
```

## Deployed contracts

**Primary:** Solana TeeProof program (see [`../clawd-verifier`](../clawd-verifier) and [`../program`](../program)). Default cluster: `devnet` (`https://api.devnet.solana.com`).

Legacy EVM (requires `--legacy-evm`; not a production default):

| Network | RedpillProofStore | Automata DCAP Verifier |
|---|---|---|
| Sepolia | `0x83541AD3f380De2b28E0108d4Da934236342B02b` | `0x76A3657F2d6c5C66733e9b69ACaDadCd0B68788b` |
| Automata Mainnet | — | `0xE26E11B257856B0bEBc4C759aaBDdea72B64351F` |

## Architecture

```
js/
  src/
    verify.ts                — verify(chatId) + verifyModel()
    providers/detect.ts      — auto-detect Phala/NearAI/Tinfoil/Chutes
    verifiers/
      cloud-api.ts           — light mode (Phala TDX + NVIDIA NRAS)
      dstack.ts              — deep mode (Docker + QEMU)
      onchain.ts             — Automata DCAP smart contracts
      tinfoil.ts             — TDX policy + SEV-SNP + Sigstore golden values
      chutes.ts              — anti-tamper binding + debug mode check
      intel-ita.ts           — Intel Trust Authority appraisal
  bin/cli.ts                 — CLI
contracts/
  RedpillProofStore.sol      — on-chain proof storage
docker-compose.yml           — dstack-verifier for deep mode
```

## Trust model

**Light mode trusts**: Phala's verification API, NVIDIA NRAS. Settlement is Solana SVM (digests only).

**Deep mode trusts**: only Intel/AMD silicon (QEMU replays boot measurements independently)

**Both modes do NOT trust**: RedPill operators, cloud providers, system administrators

## License

MIT
