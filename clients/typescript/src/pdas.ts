import { PublicKey } from '@solana/web3.js'

export const TEE_PROOF_SEED = Buffer.from('tee_proof')
export const PROOF_COUNTER_SEED = Buffer.from('proof_counter')
export const TEE_PROOF_V2_SEED = Buffer.from('tee_proof_v2')
export const PROOF_COUNTER_DAY_SEED = Buffer.from('proof_counter_day')

/**
 * Derive the PDA for a TeeProof account.
 * Seeds: ["tee_proof", quoteHash (32 bytes)]
 */
export function findTeeProofPda(
  programId: PublicKey,
  quoteHash: Uint8Array,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [TEE_PROOF_SEED, quoteHash],
    programId,
  )
}

/**
 * Derive the PDA for the ProofCounter account.
 * Seeds: ["proof_counter"]
 */
export function findProofCounterPda(programId: PublicKey): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([PROOF_COUNTER_SEED], programId)
}

/**
 * Derive the PDA for a TeeProofV2 account.
 * Seeds: ["tee_proof_v2", signedPayloadHash (32 bytes)]
 */
export function findTeeProofV2Pda(
  programId: PublicKey,
  signedPayloadHash: Uint8Array,
): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [TEE_PROOF_V2_SEED, signedPayloadHash],
    programId,
  )
}

/**
 * Derive the PDA for a sharded day counter.
 * Seeds: ["proof_counter_day", dayTag (8 bytes ASCII YYYYMMDD)]
 */
export function findProofCounterDayPda(
  programId: PublicKey,
  dayTag: string | Uint8Array,
): [PublicKey, number] {
  const seed = typeof dayTag === 'string' ? Buffer.from(dayTag, 'ascii') : dayTag
  if (seed.length !== 8) throw new Error('dayTag must be exactly 8 bytes (YYYYMMDD)')
  return PublicKey.findProgramAddressSync([PROOF_COUNTER_DAY_SEED, seed], programId)
}
