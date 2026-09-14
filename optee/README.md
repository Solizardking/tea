# CLAWD OP-TEE Signer

This directory is the CLAWD OP-TEE provider for the Solana RedPill verifier.
It gives us our own portable TEE signing boundary for Arm TrustZone systems,
while preserving the same Solana `StoreProofV2` proof format already used by
the TDX gateway.

The intent is:

1. A client app in normal world sends a 32-byte `signed_payload_hash` to OP-TEE.
2. A trusted app in secure world owns or derives the secp256k1 signing key.
3. The trusted app returns the uncompressed public key and a 65-byte recoverable
   secp256k1 signature over `keccak256(signed_payload_hash)`.
4. The gateway derives the Ethereum-style signing address from the public key,
   emits the native Solana secp256k1 verifier instruction, then stores the
   proof with `StoreProofV2`.

## Layout

```
optee/
├── include/clawd_ta.h       Shared GlobalPlatform command ABI
├── host/                    Normal-world Client API executable
├── ta/                      Secure-world trusted app skeleton
└── PROTOCOL.md              Solana proof mapping and threat model
```

## Build On A Jetson Or OP-TEE Target

The host binary needs `optee_client` headers and `libteec`. The trusted app
needs the TA dev kit exported by `optee_os`.

```bash
cd optee/host
make TEEC_EXPORT=/usr

cd ../ta
make TA_DEV_KIT_DIR=/path/to/optee_os/out/<platform>/export-ta_arm64
```

Install paths depend on the board image, but the common Jetson layout is:

```bash
sudo install -m 0755 host/clawd-optee-signer /usr/sbin/clawd-optee-signer
sudo install -m 0444 ta/4b534c41-5744-5445-452d-5349474e4552.ta /lib/optee_armtz/
```

Then sign a StoreProofV2 payload hash:

```bash
clawd-optee-signer --payload-hash 36d53db95cb277bcabfc30801ced8167d24353f5da0f5f517cb81e2c0e0dcb5c
```

The JSON output is intentionally small:

```json
{
  "provider": "optee",
  "payload_hash": "36d53db95cb277bcabfc30801ced8167d24353f5da0f5f517cb81e2c0e0dcb5c",
  "public_key": "04...",
  "signature": "..."
}
```

## Current Backend Boundary

The OP-TEE ABI and persistent secure-world key lifecycle are implemented here.
The secp256k1 backend is intentionally isolated behind:

- `clawd_k1_public_key`
- `clawd_k1_sign_recoverable`

Those functions must be wired to the board's approved secure-world k1 backend
before the TA can produce Solana-verifiable signatures. OP-TEE platforms differ:
some ship mbedTLS/TomCrypt configurations without secp256k1, and GlobalPlatform
does not guarantee recoverable secp256k1 signatures across all vendors.

For Solana compatibility, do not silently switch to P-256. The current on-chain
path uses Solana's native secp256k1 verifier, so the signature must be:

```text
r[32] || s[32] || recovery_id[1]
```

over:

```text
keccak256(signed_payload_hash[32])
```

## Relationship To The Existing TDX Gateway

The existing Rust gateway already computes `signed_payload_hash`, constructs the
Solana native secp instruction, and stores a `TeeProofV2` PDA. This OP-TEE
provider is the secure-world signer replacement for the current Rust
`TeeSigningKey` implementation:

```text
Rust in-process signer  ->  OP-TEE host CLI  ->  OP-TEE trusted app
```

The Solana proof stays stable, which means OP-TEE, TDX, and future TEE providers
can share the same on-chain verifier contract and TypeScript lookup client.
