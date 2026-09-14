/**
 * Account decoders — parse on-chain TeeProof/V2 and counter account data.
 */

import { PublicKey } from '@solana/web3.js'
import type { TeeProof, TeeProofV2 } from './types.js'
import type { VerificationLevel, ProofStatus } from './types.js'

const TEE_PROOF_DISCRIMINATOR    = 0x54 // 'T' — V1
const PROOF_COUNTER_DISCRIMINATOR = 0x43 // 'C' — V1 global counter
const TEE_PROOF_SIZE = 1 + 32 + 20 + 1 + 8 + 8 + 32 // 102

const TEE_PROOF_V2_DISCRIMINATOR    = 0x56 // 'V' — V2
const PROOF_COUNTER_DAY_DISCRIMINATOR = 0x44 // 'D' — sharded day counter
const TEE_PROOF_V2_SIZE = 372

export function decodeTeeProof(_address: PublicKey, data: Buffer): TeeProof {
  if (data.length < TEE_PROOF_SIZE) {
    throw new Error(`TeeProof account too small: ${data.length} bytes`)
  }
  if (data[0] !== TEE_PROOF_DISCRIMINATOR) {
    throw new Error(`Invalid TeeProof discriminator: 0x${data[0].toString(16)}`)
  }

  let offset = 1
  const quoteHash = new Uint8Array(data.subarray(offset, offset + 32))
  offset += 32

  const signingAddress = new Uint8Array(data.subarray(offset, offset + 20))
  offset += 20

  const isValid = data[offset] !== 0
  offset += 1

  const timestamp = data.readBigInt64LE(offset)
  offset += 8

  const slot = data.readBigUInt64LE(offset)
  offset += 8

  const submitter = new PublicKey(data.subarray(offset, offset + 32))

  return { quoteHash, signingAddress, isValid, timestamp, slot, submitter }
}

export function decodeProofCounter(data: Buffer): { count: bigint } {
  if (data.length < 9) throw new Error('ProofCounter account too small')
  if (data[0] !== PROOF_COUNTER_DISCRIMINATOR) {
    throw new Error(`Invalid ProofCounter discriminator: 0x${data[0].toString(16)}`)
  }
  const count = data.readBigUInt64LE(1)
  return { count }
}

// ---------------------------------------------------------------------------
// V2 decoders
// ---------------------------------------------------------------------------

export function decodeTeeProofV2(_address: PublicKey, data: Buffer): TeeProofV2 {
  if (data.length < TEE_PROOF_V2_SIZE) {
    throw new Error(`TeeProofV2 account too small: ${data.length} bytes (expected ${TEE_PROOF_V2_SIZE})`)
  }
  if (data[0] !== TEE_PROOF_V2_DISCRIMINATOR) {
    throw new Error(`Invalid TeeProofV2 discriminator: 0x${data[0].toString(16)}`)
  }

  let o = 1
  const version            = data[o++]
  const verificationLevel  = data[o++] as VerificationLevel
  const status             = data[o++] as ProofStatus

  const r32 = (): Uint8Array => { const v = new Uint8Array(data.subarray(o, o + 32)); o += 32; return v }

  const quoteHash             = r32()
  const signedPayloadHash     = r32()
  const requestHash           = r32()
  const responseHash          = r32()
  const modelHash             = r32()
  const attestationReportHash = r32()
  const composeHash           = r32()

  const signingAddress = new Uint8Array(data.subarray(o, o + 20)); o += 20
  const verifierSetHash = r32()
  const policyVersion = data.readUInt32LE(o); o += 4
  const nonce = r32()
  const expiresAt = data.readBigInt64LE(o); o += 8
  const timestamp = data.readBigInt64LE(o); o += 8
  const slot      = data.readBigUInt64LE(o); o += 8
  const submitter = new PublicKey(data.subarray(o, o + 32))

  return {
    version, verificationLevel, status,
    quoteHash, signedPayloadHash, requestHash, responseHash,
    modelHash, attestationReportHash, composeHash,
    signingAddress, verifierSetHash, policyVersion, nonce,
    expiresAt, timestamp, slot, submitter,
  }
}

export function decodeProofCounterDay(data: Buffer): { dayTag: string; count: bigint } {
  if (data.length < 17) throw new Error('ProofCounterDay account too small')
  if (data[0] !== PROOF_COUNTER_DAY_DISCRIMINATOR) {
    throw new Error(`Invalid ProofCounterDay discriminator: 0x${data[0].toString(16)}`)
  }
  const dayTag = data.subarray(1, 9).toString('ascii')
  const count  = data.readBigUInt64LE(9)
  return { dayTag, count }
}
