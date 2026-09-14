# CLAWD OP-TEE To Solana Protocol

## Inputs

The gateway computes the canonical V2 payload:

```text
signed_payload_hash =
  SHA256("CLAWD_REDPILL_V2:" || request_hash || response_hash || model_hash || nonce)
```

The OP-TEE host sends only this 32-byte value to the trusted app.

## Trusted App Output

The trusted app returns:

| Field | Size | Description |
|---|---:|---|
| `provider` | string | Constant `optee` |
| `payload_hash` | 32 bytes | The input hash, echoed for binding |
| `public_key` | 65 bytes | Uncompressed secp256k1 SEC1 key, `0x04 || x || y` |
| `signature` | 65 bytes | Recoverable signature, `r || s || recovery_id` |

The signature is over:

```text
keccak256(payload_hash)
```

This matches Solana's native secp256k1 program, which verifies secp signatures
against Keccak-hashed message data.

## Gateway Mapping

1. Derive the TEE signing address:

   ```text
   signing_address = keccak256(public_key[1..65])[12..32]
   ```

2. Build native Solana secp256k1 instruction with:

   ```text
   signature = r || s || recovery_id
   eth_address = signing_address
   message = payload_hash
   ```

3. Submit `StoreProofV2` with:

   ```text
   verification_level = 1
   signed_payload_hash = payload_hash
   signing_address = signing_address
   quote_hash = sha256(optee_attestation_or_binding)
   attestation_report_hash = sha256(full_platform_report)
   ```

4. Read back PDA:

   ```text
   ["tee_proof_v2", signed_payload_hash]
   ```

## Attestation Notes

OP-TEE is not a single remote-attestation product. On Arm systems, the actual
evidence path depends on the SoC, OEM boot chain, firmware configuration, and
whether the platform exposes device-unique keys or a certificate chain usable by
remote verifiers.

For Jetson-style deployments, bind the signer to:

- secure boot state and trusted OS measurement where available;
- EKB or platform sealed storage for long-term key material;
- a board- or fleet-level certificate chain for remote verifier policy;
- the `signing_address` digest inside any report data or custom evidence blob.

## Threat Model

This provider protects the signing key from normal-world Linux compromise when
the OP-TEE secure-world boundary and boot chain are intact. It does not by
itself prove a public, vendor-verifiable quote the way Intel TDX DCAP can. The
Solana proof demonstrates that a key matching the claimed TEE address signed a
payload, and the off-chain verifier policy must decide whether the OP-TEE
platform evidence is acceptable.

## Implementation Boundary

The TA in this directory creates and stores a private seed in OP-TEE private
storage, then exposes a stable command ABI. Board-specific code must connect
that seed to a secure-world secp256k1 backend before production use.
