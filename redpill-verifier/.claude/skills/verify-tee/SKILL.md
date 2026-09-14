---
name: verify-tee
description: Verify TEE attestation for Redpill/Phala Confidential AI. Supports Phala, NearAI, Tinfoil, Chutes providers. Two modes — light (no Docker) and deep (dstack-verifier + QEMU). Includes on-chain DCAP verification, ECDSA signatures, and proof storage. Use when verifying models, checking attestation, testing signatures, or storing on-chain proofs.
allowed-tools: Bash(node *), Bash(npx *), Bash(npm *), Bash(docker *), Bash(cd /tmp/redpill-verifier *)
---

# Verify TEE — RedPill Unified Verifier

Verify Confidential AI responses from any provider (Phala, NearAI, Tinfoil, Chutes).

## Setup

```bash
test -d /tmp/redpill-verifier || gh repo clone redpill-ai/redpill-verifier /tmp/redpill-verifier
cd /tmp/redpill-verifier/js && npm install && npm run build
```

For deep mode (optional):
```bash
cd /tmp/redpill-verifier && docker compose up -d
```

## Commands

### 1. Verify a chat response (main use case)

```bash
cd /tmp/redpill-verifier/js && node dist/cli.js verify <chatId> --api-key <key> --model <model>
```

### 2. Verify attestation (no API key needed)

```bash
cd /tmp/redpill-verifier/js && node dist/cli.js attestation --model <model>
```

Options:
- `--model MODEL` — default: `phala/gpt-oss-120b`
- `--deep` — force deep mode (Docker required)
- `--light` — force light mode (no Docker)
- `--network NETWORK` — `solana` (default → devnet), `devnet`, `mainnet-beta`. EVM keys require `--legacy-evm`
- `--skip-onchain` — skip legacy EVM DCAP

### 3. Store proof on-chain (Solana TeeProof)

```bash
npx clawd-verifier anchor --keypair ~/.config/solana/id.json --rpc-url https://api.devnet.solana.com
```

Deprecated EVM store: `node dist/cli.js store --legacy-evm --private-key <key> --proof-store <addr> --network sepolia`

### 4. Look up proof (Solana TeeProof)

```bash
npx clawd-verifier lookup --quote-hash <hash> --rpc-url https://api.devnet.solana.com
```

## Providers & Models

| Provider | Example models | Verification |
|---|---|---|
| **Phala** | phala/qwen-2.5-7b-instruct, phala/gpt-oss-20b | TDX + GPU + compose + deep (model+KMS+gateway) |
| **NearAI** | phala/gpt-oss-120b, phala/deepseek-chat-v3.1 | TDX + GPU + deep (model+gateway) |
| **Chutes** | phala/kimi-k2.5, phala/deepseek-v3.2 | TDX + anti-tamper binding + debug check |
| **Tinfoil** | meta-llama/llama-3.3-70b-instruct | TDX/SEV-SNP + Sigstore golden values + hw policy |

## On-chain settlement

Solana SVM settles; TEEs compute; digests only. Default cluster: `devnet`. Use `clawd-verifier` TeeProof / TeeProofV2. Ethereum / Sepolia / Automata require `--legacy-evm`.

## Interaction guidelines

1. **Ask which model** if not specified — suggest `phala/gpt-oss-120b` as default
2. **Auto-detect provider** — the verifier handles routing automatically
3. **Default to light mode** — only suggest deep mode if the user wants maximum trust guarantees
4. **Show full output** — verification results are the whole point
5. **Explain failures** — if a check fails, explain what it means for the trust model
