/**
 * @redpill-ai/solana-verifier
 *
 * Solana SVM port of the RedPill TEE attestation verifier.
 *
 * Equivalent API to the original JS verifier, with Solana on-chain storage
 * replacing the Ethereum RedpillProofStore contract.
 *
 * @example
 * ```typescript
 * import { verifyModel, lookupProof } from '@redpill-ai/solana-verifier'
 *
 * const result = await verifyModel({
 *   model: 'phala/gpt-oss-120b',
 *   solana: {
 *     rpcUrl: 'https://api.devnet.solana.com',
 *     programId: 'YOUR_PROGRAM_ID',
 *   },
 *   signer: keypair,
 * })
 *
 * console.log(result.verified)        // true
 * console.log(result.solana?.txSig)   // Solana tx signature
 * console.log(result.solana?.proofAddress.toBase58()) // PDA
 * ```
 */

export { verifyModel, verifyChatAndStore, fetchSignature, initCounterIfNeeded, storeProofOnchain, lookupProof, checkTdxQuote, checkGpu, checkReportData, selectAttestation, randomNonce, sha256, sha256Bytes, hexToBytes, computeSignedPayloadHash, storeProofV2Onchain, lookupProofV2, lookupDayCounter } from './verify.js'
export type { V2StoreFields } from './verify.js'
export { buildInitCounterInstruction, buildStoreProofInstruction, buildSecp256k1Instruction, buildStoreProofV2Instruction, buildSecp256k1InstructionV2 } from './instructions.js'
export { findTeeProofPda, findProofCounterPda, findTeeProofV2Pda, findProofCounterDayPda } from './pdas.js'
export { decodeTeeProof, decodeProofCounter, decodeTeeProofV2, decodeProofCounterDay } from './accounts.js'
export type { TeeProof, TeeProofV2, GpuResult, TdxResult, ReportDataResult, ComposeResult, SolanaOnchainResult, StoreProofOptions, StoreProofV2Options, ChatVerifyOptions, VerifyAndStoreResult, NetworkConfig, AttestationReport, RawAttestation, SignaturePayload } from './types.js'
export { VerificationLevel, ProofStatus } from './types.js'
