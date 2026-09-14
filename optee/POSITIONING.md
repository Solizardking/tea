# Public Positioning

Do not claim this is the first TEE, first OP-TEE project, first confidential AI
system, or first blockchain TEE attestation system. Those categories already
have prior work.

The defensible claim is narrower:

```text
CLAWD is a self-hosted Solana TEE proof stack that anchors verified RedPill/TDX
and OP-TEE-compatible signer evidence into the same Solana StoreProofV2 PDA
model, using native secp256k1 verification for payload binding.
```

Stronger public wording should wait for a real prior-art review. If one is
completed, keep the qualifier narrow:

```text
To our knowledge, this is one of the first Solana-native verifier paths that
combines RedPill/Phala TDX quote verification, a portable OP-TEE signer ABI,
native Solana secp256k1 verification, and PDA-addressable AI inference proofs.
```

Avoid:

```text
First TEE on blockchain.
First OP-TEE verifier.
First confidential AI verifier.
First TEE attestation on Solana.
```

Those are too broad without a formal prior-art review and are likely false or
impossible to defend.
