/**
 * Raw instruction builders — lower-level helpers for building Solana
 * transactions without the verifier orchestration.
 */

import {
  PublicKey,
  SystemProgram,
  SYSVAR_INSTRUCTIONS_PUBKEY,
  TransactionInstruction,
} from '@solana/web3.js'
import { findProofCounterDayPda, findProofCounterPda, findTeeProofPda, findTeeProofV2Pda } from './pdas.js'
import type { StoreProofV2Options, TeeProofV2 } from './types.js'

/** Build the InitCounter instruction (one-time program setup). */
export function buildInitCounterInstruction(
  programId: PublicKey,
  payer: PublicKey,
): TransactionInstruction {
  const [counterPda] = findProofCounterPda(programId)
  const data = Buffer.from([0]) // discriminator 0 = InitCounter

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: payer, isSigner: true, isWritable: true },
      { pubkey: counterPda, isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    ],
    data,
  })
}

/** Build the StoreProof instruction. */
export function buildStoreProofInstruction(
  programId: PublicKey,
  payer: PublicKey,
  quoteHash: Uint8Array,       // 32 bytes
  signingAddress: Uint8Array,  // 20 bytes
  isValid: boolean,
  verifySig = false,
): TransactionInstruction {
  if (quoteHash.length !== 32) throw new Error('quoteHash must be 32 bytes')
  if (signingAddress.length !== 20) throw new Error('signingAddress must be 20 bytes')

  const [proofPda] = findTeeProofPda(programId, quoteHash)
  const [counterPda] = findProofCounterPda(programId)

  // 1 byte discriminator + 32 + 20 + 1 + 1 = 55 bytes
  const ixData = Buffer.alloc(55)
  ixData[0] = 1 // discriminator = StoreProof
  Buffer.from(quoteHash).copy(ixData, 1)
  Buffer.from(signingAddress).copy(ixData, 33)
  ixData[53] = isValid ? 1 : 0
  ixData[54] = verifySig ? 1 : 0

  const keys = [
    { pubkey: payer, isSigner: true, isWritable: true },
    { pubkey: proofPda, isSigner: false, isWritable: true },
    { pubkey: counterPda, isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
  ]

  if (verifySig) {
    keys.push({ pubkey: SYSVAR_INSTRUCTIONS_PUBKEY, isSigner: false, isWritable: false })
  }

  return new TransactionInstruction({ programId, keys, data: ixData })
}

/**
 * Build a native secp256k1 verify instruction.
 *
 * When `verifySig = true` in StoreProof, this instruction MUST appear at
 * index 0 in the transaction. The on-chain program reads the instruction
 * sysvar and confirms the Ethereum address matches `signing_address`.
 *
 * @param ethAddress  20-byte Ethereum address (derived from TEE ECDSA pubkey)
 * @param message     The message that was signed (text/bytes)
 * @param signature   65-byte signature (r[32] + s[32] + v[1])
 */
export function buildSecp256k1Instruction(
  ethAddress: Uint8Array,
  message: Uint8Array,
  signature: Uint8Array,
): TransactionInstruction {
  // secp256k1 native program ID
  const SECP256K1_PROGRAM_ID = new PublicKey('KeccakSecp256k11111111111111111111111111111')

  if (ethAddress.length !== 20) throw new Error('ethAddress must be 20 bytes')
  if (signature.length !== 65) throw new Error('signature must be 65 bytes')

  // Header layout (11 bytes per signature):
  //   sig_offset(2) + sig_ix(1) + addr_offset(2) + addr_ix(1)
  //   + msg_offset(2) + msg_size(2) + msg_ix(1)
  const sigCount = 1
  const headerSize = 1 + sigCount * 11

  // Data layout: header + signature(64) + recovery_id(1) + eth_address(20) + message
  const sigOffset = headerSize                 // offset of signature in data
  const addrOffset = sigOffset + 65            // offset of eth_address
  const msgOffset = addrOffset + 20            // offset of message
  const msgSize = message.length

  const dataLen = msgOffset + msgSize
  const buf = Buffer.alloc(dataLen)

  buf[0] = sigCount

  // Header for signature 0 (little-endian u16 offsets)
  buf.writeUInt16LE(sigOffset, 1)   // sig_offset
  buf[3] = 0xff                     // sig_instruction_ix = 0xff (current instruction)
  buf.writeUInt16LE(addrOffset, 4)  // eth_addr_offset
  buf[6] = 0xff                     // eth_addr_instruction_ix = 0xff
  buf.writeUInt16LE(msgOffset, 7)   // msg_data_offset
  buf.writeUInt16LE(msgSize, 9)     // msg_data_size
  buf[11] = 0xff                    // msg_instruction_ix = 0xff

  // Payload
  Buffer.from(signature).copy(buf, sigOffset)
  Buffer.from(ethAddress).copy(buf, addrOffset)
  Buffer.from(message).copy(buf, msgOffset)

  return new TransactionInstruction({
    programId: SECP256K1_PROGRAM_ID,
    keys: [],
    data: buf,
  })
}

// ---------------------------------------------------------------------------
// V2 instruction builders
// ---------------------------------------------------------------------------

const SECP256K1_PROGRAM_ID_V2 = new PublicKey('KeccakSecp256k11111111111111111111111111111')

/**
 * Build a secp256k1 verify instruction for V2.
 *
 * The message MUST be exactly the 32-byte `signedPayloadHash` — the on-chain
 * program will reject any secp instruction whose message doesn't match.
 *
 * Place this instruction at the index you pass as `secpIxIndex` to
 * `buildStoreProofV2Instruction`. Compute Budget instructions may appear before it.
 */
export function buildSecp256k1InstructionV2(
  ethAddress: Uint8Array,   // 20 bytes
  signedPayloadHash: Uint8Array, // 32 bytes — the exact bytes that were signed
  signature: Uint8Array,    // 65 bytes: r(32)+s(32)+v(1)
): TransactionInstruction {
  if (ethAddress.length !== 20) throw new Error('ethAddress must be 20 bytes')
  if (signedPayloadHash.length !== 32) throw new Error('signedPayloadHash must be 32 bytes')
  if (signature.length !== 65) throw new Error('signature must be 65 bytes')

  const sigCount = 1
  const headerSize = 1 + sigCount * 11
  const sigOffset = headerSize
  const addrOffset = sigOffset + 65
  const msgOffset = addrOffset + 20
  const msgSize = 32

  const buf = Buffer.alloc(msgOffset + msgSize)
  buf[0] = sigCount
  buf.writeUInt16LE(sigOffset, 1)
  buf[3] = 0xff
  buf.writeUInt16LE(addrOffset, 4)
  buf[6] = 0xff
  buf.writeUInt16LE(msgOffset, 7)
  buf.writeUInt16LE(msgSize, 9)
  buf[11] = 0xff

  Buffer.from(signature).copy(buf, sigOffset)
  Buffer.from(ethAddress).copy(buf, addrOffset)
  Buffer.from(signedPayloadHash).copy(buf, msgOffset)

  return new TransactionInstruction({ programId: SECP256K1_PROGRAM_ID_V2, keys: [], data: buf })
}

/**
 * Build the StoreProofV2 instruction (discriminator = 2).
 *
 * Instruction data layout after the discriminator byte:
 *   secp_ix_index(2) + verification_level(1)
 *   + signed_payload_hash(32) + quote_hash(32) + request_hash(32)
 *   + response_hash(32) + model_hash(32) + attestation_report_hash(32) + compose_hash(32)
 *   + signing_address(20) + verifier_set_hash(32) + policy_version(4)
 *   + nonce(32) + expires_at(8) + day_tag(8)
 */
export function buildStoreProofV2Instruction(
  programId: PublicKey,
  payer: PublicKey,
  proof: Pick<TeeProofV2,
    | 'verificationLevel' | 'signedPayloadHash' | 'quoteHash'
    | 'requestHash' | 'responseHash' | 'modelHash'
    | 'attestationReportHash' | 'composeHash'
    | 'signingAddress' | 'verifierSetHash' | 'policyVersion'
    | 'nonce' | 'expiresAt'
  >,
  opts: StoreProofV2Options = {},
): TransactionInstruction {
  const secpIxIndex = opts.secpIxIndex ?? 0
  const dayTag = opts.dayTag ?? new Date().toISOString().slice(0, 10).replace(/-/g, '')

  const [proofPda] = findTeeProofV2Pda(programId, proof.signedPayloadHash)
  const [dayCounterPda] = findProofCounterDayPda(programId, dayTag)

  // 1 discriminator + 331 data bytes = 332 total
  const ixData = Buffer.alloc(332)
  let off = 0
  ixData[off++] = 2 // discriminator = StoreProofV2
  ixData.writeUInt16LE(secpIxIndex, off); off += 2
  ixData[off++] = proof.verificationLevel

  const write32 = (src: Uint8Array) => { Buffer.from(src).copy(ixData, off); off += 32 }
  write32(proof.signedPayloadHash)
  write32(proof.quoteHash)
  write32(proof.requestHash)
  write32(proof.responseHash)
  write32(proof.modelHash)
  write32(proof.attestationReportHash)
  write32(proof.composeHash)

  Buffer.from(proof.signingAddress).copy(ixData, off); off += 20
  write32(proof.verifierSetHash)
  ixData.writeUInt32LE(proof.policyVersion, off); off += 4
  write32(proof.nonce)

  const expiresAtBuf = Buffer.alloc(8)
  expiresAtBuf.writeBigInt64LE(proof.expiresAt); expiresAtBuf.copy(ixData, off); off += 8

  Buffer.from(dayTag, 'ascii').copy(ixData, off) // 8 bytes

  return new TransactionInstruction({
    programId,
    keys: [
      { pubkey: payer,                          isSigner: true,  isWritable: true },
      { pubkey: proofPda,                       isSigner: false, isWritable: true },
      { pubkey: dayCounterPda,                  isSigner: false, isWritable: true },
      { pubkey: SystemProgram.programId,        isSigner: false, isWritable: false },
      { pubkey: SYSVAR_INSTRUCTIONS_PUBKEY,     isSigner: false, isWritable: false },
    ],
    data: ixData,
  })
}
