# SVM Handoff: Solana TEE Verifier

```
░░ HISTORICAL V1 HAND·OFF · ETHEREUM → SOLANA ░░
░░ LIVE TRUTH → root README.md (V2 · gateway · SAS · OP-TEE) ░░
```

**Historical V1 handoff.** This document explains the original Ethereum-to-Solana storage port. The root [`README.md`](./README.md) is the source of truth for the current **V2**, gateway, SAS, OP-TEE stack, monorepo map, and doctrine under [`content/`](./content/).

Doctrine companions:

| Doc | Role |
| --- | --- |
| [`README.md`](./README.md) | Cyber map · V2 · full tree |
| [`content/soul/SOUL.md`](./content/soul/SOUL.md) | Relay character harness |
| [`content/articles/clawd-solana-svm-design.md`](./content/articles/clawd-solana-svm-design.md) | Grid / Forge / Relay / Registry / Verify |
| [`content/articles/brave-new-world-blockchain-ai.md`](./content/articles/brave-new-world-blockchain-ai.md) | Provenance · ORL · Solana coordination |

---

## What was built

The original [`redpill-verifier/`](./redpill-verifier) is a TypeScript + Python toolkit that verifies AI responses came from genuine TEE hardware (Intel TDX / NVIDIA Confidential Computing). It stores verification proof records on Ethereum via a Solidity contract (`RedpillProofStore.sol`).

We rewrote the on-chain storage layer to target **Solana SVM** instead of Ethereum. The off-chain verification chain (Phala TDX API, NVIDIA NRAS, report data binding) is unchanged — only the proof storage destination changed.

**Live layout today:** workspace root (`program/`, `clients/`, `clawd-tee-gateway/`, …) **and** nested mirror [`solana-tee/`](./solana-tee). Paths below use the root / nested package shape (formerly `web/solana-tee/`).

---

## Repository layout (V1-era program + client)

```
clawd-tee/  (or solana-tee/ nested mirror)
├── Cargo.toml                          # workspace root (pinocchio 0.9, solana-program 2.1)
├── program/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs                      # module declarations
│       ├── entrypoint.rs               # dispatch table (discriminators 0, 1, … later 2 in V2)
│       ├── constants.rs                # seeds, sizes, discriminators
│       ├── error.rs                    # RedpillVerifierError enum
│       ├── state/
│       │   ├── tee_proof.rs            # TeeProof account (102 bytes)
│       │   └── proof_counter.rs        # ProofCounter account (9 bytes)
│       └── processor/
│           ├── init_counter.rs         # instruction 0: InitCounter
│           ├── store_proof.rs          # instruction 1: StoreProof
│           └── shared/
│               ├── account_checks.rs   # signer / writable / owner asserts
│               └── pda_utils.rs        # PDA derivation + create_pda_account
├── clients/typescript/
│   ├── package.json                    # @redpill-ai/solana-verifier
│   ├── tsconfig.json
│   └── src/
│       ├── index.ts                    # re-exports everything
│       ├── types.ts                    # all TypeScript types
│       ├── pdas.ts                     # findTeeProofPda, findProofCounterPda
│       ├── accounts.ts                 # decodeTeeProof, decodeProofCounter
│       ├── instructions.ts             # buildInitCounterInstruction, buildStoreProofInstruction, buildSecp256k1Instruction
│       └── verify.ts                   # verifyModel(), storeProofOnchain(), lookupProof()
└── README.md                           # live map + V2 (root)
```

---

## Ethereum → Solana mapping (exact)

| Ethereum | Solana |
|---|---|
| `RedpillProofStore.sol` | `program/src/processor/store_proof.rs` |
| `mapping(bytes32 => Proof)` | One `TeeProof` PDA per `quote_hash` |
| `verifyAndStore(quote, addr)` | `StoreProof` instruction (discriminator `1`) |
| `getProof(quoteHash)` | `lookupProof()` → `getAccountInfo(pda)` |
| `ProofStored` event | Account write (queryable via RPC) |
| `keccak256(quote)` | `sha256(quote)` |
| `block.timestamp` | `Clock::unix_timestamp` |
| `block.number` | `Clock::slot` |
| `msg.sender` | Signer `Pubkey` |
| `DCAP verifyAndAttestOnChain()` | Off-chain (same Phala/Automata APIs) |
| ERC-style gas | SOL rent + tx fee |

---

## On-chain accounts

### `TeeProof` PDA

Seeds: `["tee_proof", quote_hash[0..32]]`  
Size: 102 bytes  
Discriminator: `0x54`

| Offset | Field | Type | Notes |
|---|---|---|---|
| 0 | discriminator | `u8` | `0x54` |
| 1 | `quote_hash` | `[u8;32]` | SHA256 of raw TDX quote bytes |
| 33 | `signing_address` | `[u8;20]` | Ethereum-style TEE signing address |
| 53 | `is_valid` | `u8` | 1 = DCAP passed off-chain |
| 54 | `timestamp` | `i64 LE` | Unix seconds from Clock sysvar |
| 62 | `slot` | `u64 LE` | Solana slot |
| 70 | `submitter` | `[u8;32]` | Pubkey of tx signer |

### `ProofCounter` PDA

Seeds: `["proof_counter"]`  
Size: 9 bytes  
Discriminator: `0x43`  
Tracks total proofs stored globally.

---

## Program instructions

### Instruction 0 — `InitCounter`

**One-time call.** Initializes the `ProofCounter` PDA.

Accounts:
```
0. payer          [signer, writable]
1. counter_account [writable]         PDA: ["proof_counter"]
2. system_program  []
```

Instruction data: `[0x00]` (1-byte discriminator only)

---

### Instruction 1 — `StoreProof`

Stores a TEE attestation proof record permanently.

Accounts:
```
0. payer          [signer, writable]
1. proof_account  [writable]          PDA: ["tee_proof", quote_hash]
2. counter_account [writable]         PDA: ["proof_counter"]
3. system_program  []
4. instructions_sysvar []             only required when verify_sig = 1
```

Instruction data (after 1-byte discriminator `0x01`):

| Offset | Field | Size | Description |
|---|---|---|---|
| 0 | `quote_hash` | 32 | SHA256 of TDX quote bytes |
| 32 | `signing_address` | 20 | Ethereum-style address |
| 52 | `is_valid` | 1 | `0x01` if DCAP passed |
| 53 | `verify_sig` | 1 | `0x01` to assert secp256k1 ix at index 0 |

Total instruction data size (excluding discriminator): 54 bytes.

---

## Secp256k1 on-chain signature verification

When `verify_sig = 1`, the program reads the `Instructions` sysvar and confirms:
1. Instruction 0 in the transaction is a secp256k1 native program instruction.
2. The recovered Ethereum address in that instruction matches `signing_address`.

To use this, the transaction must look like:

```
Transaction:
  ix[0]: secp256k1 native program (KeccakSecp256k11111111111111111111111111111)
         — signs sha256(request):sha256(response) with TEE ECDSA key
  ix[1]: StoreProof (discriminator 1, verify_sig = 1)
```

The `buildSecp256k1Instruction()` helper in `clients/typescript/src/instructions.ts` builds the secp256k1 instruction with the correct 11-byte header layout.

---

## Integration steps for the implementing agent

### 1. Deploy the program

```bash
# workspace root or nested solana-tee/
cd /path/to/clawd-tee   # or: cd solana-tee
cargo build-sbf
solana program deploy target/deploy/solana_redpill_verifier.so --url devnet
# → saves Program ID e.g. "RedPi11....."
```

### 2. Run InitCounter (once per deployment)

```typescript
import { buildInitCounterInstruction } from '@redpill-ai/solana-verifier'
import { Connection, Keypair, Transaction, sendAndConfirmTransaction } from '@solana/web3.js'

const programId = new PublicKey('YOUR_DEPLOYED_PROGRAM_ID')
const ix = buildInitCounterInstruction(programId, wallet.publicKey)
await sendAndConfirmTransaction(connection, new Transaction().add(ix), [wallet])
```

### 3. Verify TEE + store on Solana

```typescript
import { verifyModel } from '@redpill-ai/solana-verifier'

const result = await verifyModel({
  model: 'phala/gpt-oss-120b',
  solana: {
    rpcUrl: 'https://api.devnet.solana.com',
    programId: 'YOUR_DEPLOYED_PROGRAM_ID',
  },
  signer: wallet, // Keypair
})

// result.verified       — true if TDX + GPU + report_data all passed
// result.solana.txSig   — Solana transaction signature
// result.solana.proofAddress.toBase58() — the TeeProof PDA address
```

### 4. Look up a stored proof (no wallet needed)

```typescript
import { lookupProof } from '@redpill-ai/solana-verifier'

const proof = await lookupProof(
  { rpcUrl: 'https://api.devnet.solana.com', programId: 'YOUR_DEPLOYED_PROGRAM_ID' },
  quoteHashBytes, // Uint8Array[32] — sha256 of the TDX quote
)

console.log(proof?.isValid)    // true
console.log(proof?.timestamp)  // BigInt unix timestamp
console.log(proof?.submitter)  // PublicKey
```

### 5. Wiring secp256k1 (optional but recommended for trustless mode)

```typescript
import { buildSecp256k1Instruction, buildStoreProofInstruction } from '@redpill-ai/solana-verifier'

const secp256k1Ix = buildSecp256k1Instruction(
  ethAddressBytes,   // Uint8Array[20]
  messageBytes,      // Uint8Array — the signed message
  signatureBytes,    // Uint8Array[65] — r(32) + s(32) + v(1)
)

const storeIx = buildStoreProofInstruction(
  programId,
  wallet.publicKey,
  quoteHashBytes,
  ethAddressBytes,
  isValid,
  /* verifySig= */ true,
)

// secp256k1 instruction MUST be at index 0
const tx = new Transaction().add(secp256k1Ix, storeIx)
await sendAndConfirmTransaction(connection, tx, [wallet])
```

---

## Environment variables needed

```bash
SOLANA_RPC_URL=https://api.devnet.solana.com
REDPILL_PROGRAM_ID=<deployed program ID>
REDPILL_API_KEY=<provider-api-key> # for verify() signature fetching
# Wallet keypair loaded from file or env for submitter
```

---

## Trust model

**Off-chain (unchanged from original):**
- Phala TDX verifier API → validates Intel TDX quote
- NVIDIA NRAS → validates GPU is genuine H100/H200 in CC mode
- Report data binding → signing_address + nonce embedded in TDX quote

**On-chain (Solana, new):**
- `is_valid` is submitted by the caller (trusted submitter model)
- When `verify_sig = 1`: secp256k1 native program cryptographically proves the TEE ECDSA key signed the payload — this is the trustless path
- Anyone can read any `TeeProof` PDA and independently re-verify by re-fetching the TDX quote from RedPill API and re-running the cloud verifiers

**Does NOT trust:** RedPill operators, cloud providers, or the submitter (in secp256k1 mode).

---

## What still needs to be done

1. **Deploy to devnet/mainnet** — `cargo-build-sbf` then `solana program deploy`
2. **Wire into the main zk-router gate flow** — after a user's AI request is verified by the TEE, call `verifyModel()` and surface `result.solana.txSig` in the `X-ZK-Proof` response header
3. **Add a `/api/proof/lookup` route** in the Next.js app — accepts `?quoteHash=0x...`, calls `lookupProof()`, returns the `TeeProof` fields as JSON
4. **IDL generation** (optional) — run `shank idl` to generate a JSON IDL for Codama-based client regeneration
5. **Integration tests** — use `solana-program-test` in `program/` dev-dependencies (already wired in `Cargo.toml`)
6. **Consider Automata DCAP on Solana** — if Automata ships a Solana port of their DCAP verifier program, the `is_valid` flag can be replaced with a CPI call to that program, making it fully trustless end-to-end

---

## File-by-file quick reference

| File | What to edit | Why |
|---|---|---|
| `program/src/constants.rs` | `TEE_PROOF_SEED` | Change PDA namespace if needed |
| `program/src/processor/store_proof.rs` | `verify_secp256k1_present()` | Tighten the program_id check against the secp256k1 program ID bytes |
| `clients/typescript/src/verify.ts` | `storeProofOnchain()` | Add transaction retry logic, priority fees |
| `clients/typescript/src/verify.ts` | `verifyModel()` | Add Tinfoil / Chutes / NearAI provider-specific checks (same as original JS verifier) |
| `clients/typescript/src/instructions.ts` | `buildSecp256k1Instruction()` | Already correct — secp256k1 header layout matches Solana native program spec |
