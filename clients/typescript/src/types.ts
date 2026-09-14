import type { PublicKey, TransactionInstruction } from '@solana/web3.js'

// ---------------------------------------------------------------------------
// On-chain account state (mirrors TeeProof Rust struct)
// ---------------------------------------------------------------------------

export interface TeeProof {
  quoteHash: Uint8Array        // 32 bytes — SHA256 of TDX quote
  signingAddress: Uint8Array   // 20 bytes — Ethereum-style address
  isValid: boolean
  timestamp: bigint            // Unix seconds (i64)
  slot: bigint                 // Solana slot (u64)
  submitter: PublicKey
}

// ---------------------------------------------------------------------------
// Off-chain verification results (mirrors the JS verifier types)
// ---------------------------------------------------------------------------

export interface TdxResult {
  verified: boolean
  quote?: {
    body?: {
      reportdata?: string
      mrconfig?: string
    }
  }
  message?: string
}

export interface GpuResult {
  nonceMatches: boolean
  verdict: string             // "true" | "PASS" | etc.
}

export interface ReportDataResult {
  bindsAddress: boolean
  embedsNonce: boolean
}

export interface ComposeResult {
  hashMatches: boolean
  composeHash: string
  mrConfig: string
}

export interface SolanaOnchainResult {
  /** Solana transaction signature */
  txSig: string
  /** PDA where the proof is stored */
  proofAddress: PublicKey
  slot: bigint
  /** True when a matching proof PDA already existed and no store tx was sent. */
  alreadyExisted?: boolean
}

export interface VerifyAndStoreResult {
  verified: boolean
  model: string
  provider: string
  signingAddress: string | null
  /** SHA256 of the raw Intel quote bytes. Present when an attestation quote was available. */
  quoteHash: string | null
  nonce: string
  tdx: TdxResult | null
  gpu: GpuResult | null
  reportData: ReportDataResult | null
  compose: ComposeResult | null
  /** Solana on-chain storage result */
  solana: SolanaOnchainResult | null
  errors: string[]
}

export interface StoreProofOptions {
  /** Add a native secp256k1 instruction before StoreProof and set verify_sig=1. */
  verifySig?: boolean
  /** Message bytes or UTF-8 text signed by the Redpill TEE ECDSA key. */
  signedMessage?: Uint8Array | string
  /** 65-byte secp256k1 signature: r(32) + s(32) + recovery id(1). */
  signature?: Uint8Array
  /** Initialize the ProofCounter PDA if it is missing. Defaults to true. */
  initCounter?: boolean
  /** Return the existing proof instead of failing if the proof PDA already exists. Defaults to true. */
  allowExisting?: boolean
}

// ---------------------------------------------------------------------------
// V2 types
// ---------------------------------------------------------------------------

/** Mirrors VERIFICATION_LEVEL_* constants in constants.rs */
export const VerificationLevel = {
  StoredOnly:          0,
  TeeSigOnchain:       1,
  DcapOffchain:        2,
  MultiVerifier:       3,
  DcapOnchain:         4,
  ZkAuthorized:        5,
} as const
export type VerificationLevel = typeof VerificationLevel[keyof typeof VerificationLevel]

/** Mirrors PROOF_STATUS_* constants in constants.rs */
export const ProofStatus = {
  Active:   0,
  Consumed: 1,
  Revoked:  2,
  Expired:  3,
} as const
export type ProofStatus = typeof ProofStatus[keyof typeof ProofStatus]

/** Mirrors TeeProofV2 Rust struct */
export interface TeeProofV2 {
  version: number                      // u8 — always 2
  verificationLevel: VerificationLevel // u8
  status: ProofStatus                  // u8
  quoteHash: Uint8Array                // 32 bytes
  signedPayloadHash: Uint8Array        // 32 bytes — the hash the secp256k1 sig covers
  requestHash: Uint8Array              // 32 bytes
  responseHash: Uint8Array             // 32 bytes
  modelHash: Uint8Array                // 32 bytes
  attestationReportHash: Uint8Array    // 32 bytes
  composeHash: Uint8Array              // 32 bytes
  signingAddress: Uint8Array           // 20 bytes
  verifierSetHash: Uint8Array          // 32 bytes
  policyVersion: number                // u32
  nonce: Uint8Array                    // 32 bytes
  expiresAt: bigint                    // i64 unix seconds (0 = no expiry)
  timestamp: bigint                    // i64 unix seconds
  slot: bigint                         // u64
  submitter: PublicKey
}

/** Options for buildStoreProofV2Instruction */
export interface StoreProofV2Options {
  /** Optional instructions to prepend before secp256k1 and StoreProofV2. */
  preInstructions?: TransactionInstruction[]
  /**
   * Index of the secp256k1 instruction in the transaction.
   * Set to the actual position (e.g. 2 if two Compute Budget ixs precede it).
   * Only required when verificationLevel >= TeeSigOnchain.
   */
  secpIxIndex?: number
  /** YYYYMMDD ASCII string (8 bytes) for the sharded day counter PDA seed. */
  dayTag?: string
  /** 65-byte signature for the secp256k1 instruction. */
  signature?: Uint8Array
}

export interface NetworkConfig {
  /** Solana RPC endpoint */
  rpcUrl: string
  /** Program ID of the deployed solana-redpill-verifier program */
  programId: string
}

export interface ChatVerifyOptions {
  requestId: string
  model: string
  apiKey: string
  solana?: NetworkConfig
  signer?: import('@solana/web3.js').Keypair
  requestBody?: string
  responseText?: string
  skipGpu?: boolean
  skipTdx?: boolean
  store?: StoreProofOptions | boolean
}

// RedPill API types
export interface AttestationReport {
  error?: string
  signing_address?: string
  intel_quote?: string
  nvidia_payload?: string
  signing_algo?: string
  info?: { tcb_info?: unknown }
  event_log?: unknown
  model_attestations?: RawAttestation[]
  all_attestations?: RawAttestation[]
  gateway_attestation?: RawAttestation
}

export interface RawAttestation {
  signing_address?: string
  intel_quote?: string
  nvidia_payload?: string
  signing_algo?: string
  info?: { tcb_info?: unknown }
  event_log?: unknown
  nonce?: string
  e2e_pubkey?: string
  instance_id?: string
}

export interface SignaturePayload {
  error?: string
  text: string
  signature: string
  signing_address: string
  signing_algo?: string
}
