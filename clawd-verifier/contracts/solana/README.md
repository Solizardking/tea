# Solana verifier program

`solana_redpill_verifier.json` is the IDL for the CLAWD Solana verifier program.

The program source lives in [`../../solana-redpill-verifier/program`](../../solana-redpill-verifier/program)
(native Rust, no Anchor). It is not duplicated here — this package's JS client
(`js/src/solana`) talks to it directly via the IDL-described instructions:

| Instruction | Purpose |
|---|---|
| `InitCounter` | Create the global proof counter PDA |
| `StoreProof` | V1: store quote hash + signing address + off-chain verdict |
| `StoreProofV2` | V2: full hash set, verification levels 0–5, optional on-chain secp256k1 verification via the native precompile, sharded day counters |

Build and deploy from the program workspace:

```bash
cd ../../solana-redpill-verifier
cargo build-sbf -- -p solana-redpill-verifier
solana program deploy target/deploy/solana_redpill_verifier.so
```

Local deployment (validator at `http://127.0.0.1:8899`):
program ID `HvXe5RpCcduVYJWDyavmNKde4uEgUFEiYz99JYQT4DkP`.
