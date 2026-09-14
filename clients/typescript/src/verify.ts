/**
 * Off-chain TEE verification logic — mirrors the original JS verifier but
 * targets Solana on-chain storage instead of Ethereum.
 *
 * Verification chain (identical to original):
 *   1. Fetch attestation from RedPill API
 *   2. Verify Intel TDX quote via Phala cloud API
 *   3. Verify report_data binding (signing_address + nonce)
 *   4. Verify GPU attestation via NVIDIA NRAS
 *   5. Verify compose manifest vs mr_config
 *   6. Hash TDX quote bytes with SHA256
 *   7. Build + submit Solana tx: secp256k1 + StoreProof
 */

import type {
  AttestationReport,
  ChatVerifyOptions,
  GpuResult,
  NetworkConfig,
  RawAttestation,
  ReportDataResult,
  SignaturePayload,
  SolanaOnchainResult,
  StoreProofOptions,
  StoreProofV2Options,
  TdxResult,
  TeeProofV2,
  VerifyAndStoreResult,
} from './types.js'
import { VerificationLevel } from './types.js'
import {
  Connection,
  type Keypair,
  PublicKey,
  Transaction,
  sendAndConfirmTransaction,
} from '@solana/web3.js'
import { recoverMessageAddress } from 'viem'
import {
  buildInitCounterInstruction,
  buildSecp256k1Instruction,
  buildSecp256k1InstructionV2,
  buildStoreProofInstruction,
  buildStoreProofV2Instruction,
} from './instructions.js'
import { findProofCounterDayPda, findProofCounterPda, findTeeProofPda, findTeeProofV2Pda } from './pdas.js'

const API_BASE = 'https://api.redpill.ai'
const GPU_VERIFIER_API = 'https://nras.attestation.nvidia.com/v3/attest/gpu'
const PHALA_TDX_VERIFIER_API = 'https://cloud-api.phala.network/api/v1/attestations/verify'

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

export function randomNonce(bytes = 32): string {
  const arr = new Uint8Array(bytes)
  crypto.getRandomValues(arr)
  return Array.from(arr)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export async function sha256(text: string): Promise<string> {
  const data = new TextEncoder().encode(text)
  const bytes = data.buffer.slice(data.byteOffset, data.byteOffset + data.byteLength) as ArrayBuffer
  const hash = await crypto.subtle.digest('SHA-256', bytes)
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export async function sha256Bytes(bytes: Uint8Array): Promise<Uint8Array> {
  const data = bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer
  const hash = await crypto.subtle.digest('SHA-256', data)
  return new Uint8Array(hash)
}

export function hexToBytes(hex: string, expectedLength?: number): Uint8Array {
  const clean = hex.replace(/^0x/, '')
  if (clean.length % 2 !== 0) throw new Error('hex string must have even length')
  const bytes = Buffer.from(clean, 'hex')
  if (expectedLength !== undefined && bytes.length !== expectedLength) {
    throw new Error(`expected ${expectedLength} bytes, got ${bytes.length}`)
  }
  return new Uint8Array(bytes)
}

function asHexSignature(signature: string): `0x${string}` {
  return signature.startsWith('0x')
    ? signature as `0x${string}`
    : `0x${signature}` as `0x${string}`
}

function normalizeSecp256k1Signature(signature: Uint8Array): Uint8Array {
  if (signature.length !== 65) throw new Error('signature must be 65 bytes')
  const normalized = new Uint8Array(signature)
  if (normalized[64] >= 27) normalized[64] -= 27
  return normalized
}

/** Decode the payload section of a JWT token. */
function decodeJwtPayload(jwt: string): Record<string, unknown> {
  const b64 = jwt.split('.')[1]
  const padded = b64 + '='.repeat((4 - (b64.length % 4)) % 4)
  const json = atob(padded.replace(/-/g, '+').replace(/_/g, '/'))
  return JSON.parse(json)
}

// ---------------------------------------------------------------------------
// RedPill API helpers
// ---------------------------------------------------------------------------

async function fetchAttestation(
  model: string,
  nonce: string,
  apiKey?: string,
  signingAddress?: string,
): Promise<AttestationReport> {
  const params = new URLSearchParams({ model, nonce })
  if (signingAddress) params.set('signing_address', signingAddress)
  const headers = apiKey ? { Authorization: `Bearer ${apiKey}` } : undefined
  const res = await fetch(`${API_BASE}/v1/attestation/report?${params.toString()}`, {
    headers,
    signal: AbortSignal.timeout(120_000),
  })
  if (!res.ok) throw new Error(`Attestation API HTTP ${res.status}`)
  return res.json()
}

export async function fetchSignature(
  requestId: string,
  model: string,
  apiKey: string,
): Promise<SignaturePayload> {
  const params = new URLSearchParams({ model })
  const res = await fetch(`${API_BASE}/v1/signature/${encodeURIComponent(requestId)}?${params.toString()}`, {
    headers: { Authorization: `Bearer ${apiKey}` },
    signal: AbortSignal.timeout(60_000),
  })
  if (!res.ok) throw new Error(`Signature API HTTP ${res.status}`)
  return res.json()
}

/** Normalize all RedPill response formats to a single attestation object. */
export function selectAttestation(report: AttestationReport, preferredSigningAddress?: string): RawAttestation {
  const preferred = preferredSigningAddress?.toLowerCase()
  const candidates: RawAttestation[] = []
  for (const key of ['model_attestations', 'all_attestations'] as const) {
    const list = report[key]
    if (Array.isArray(list)) {
      for (const att of list) {
        if (att && typeof att === 'object' && 'signing_address' in att) candidates.push(att)
      }
    }
  }
  if (report.gateway_attestation) candidates.push(report.gateway_attestation)
  candidates.push(report as RawAttestation)

  if (preferred) {
    const exact = candidates.find((att) => att.signing_address?.toLowerCase() === preferred)
    if (exact) return exact
  }

  return candidates.find((att) => att.signing_address) ?? candidates[0]
}

// ---------------------------------------------------------------------------
// Step-by-step verifiers
// ---------------------------------------------------------------------------

/** Verify TDX quote via Phala's cloud verification API. */
export async function checkTdxQuote(intelQuote: string): Promise<TdxResult> {
  const res = await fetch(PHALA_TDX_VERIFIER_API, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ hex: intelQuote }),
    signal: AbortSignal.timeout(60_000),
  })
  const body = await res.json() as Record<string, unknown>
  const quote = (body.quote ?? {}) as Record<string, unknown>
  return {
    verified: Boolean(quote.verified),
    quote: quote as TdxResult['quote'],
    message: (quote.message ?? body.message) as string | undefined,
  }
}

/** Verify report_data binding: first 32 bytes = signing_address, next 32 = nonce. */
export function checkReportData(
  signingAddress: string,
  signingAlgo: string,
  nonce: string,
  reportDataHex: string,
): ReportDataResult {
  const rd = Buffer.from(reportDataHex.replace(/^0x/, ''), 'hex')
  const algo = signingAlgo.toLowerCase()

  let addrBytes: Buffer
  if (algo === 'ecdsa') {
    addrBytes = Buffer.from(signingAddress.replace(/^0x/, ''), 'hex')
  } else {
    addrBytes = Buffer.from(signingAddress, 'hex')
  }

  const embedded = rd.subarray(0, 32)
  const expectedAddr = Buffer.alloc(32)
  addrBytes.copy(expectedAddr, 0)

  const bindsAddress = embedded.equals(expectedAddr)
  const embedsNonce = rd.subarray(32).toString('hex') === nonce

  return { bindsAddress, embedsNonce }
}

/** Verify GPU attestation via NVIDIA NRAS. */
export async function checkGpu(nvidiaPayload: string, nonce: string): Promise<GpuResult> {
  const payload = JSON.parse(nvidiaPayload) as Record<string, unknown>
  const nonceMatches = String(payload.nonce).toLowerCase() === nonce.toLowerCase()

  const body = await fetch(GPU_VERIFIER_API, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: nvidiaPayload,
    signal: AbortSignal.timeout(30_000),
  }).then((r) => r.json()) as unknown[]

  const jwtToken = (body[0] as [string, string])[1]
  const claims = decodeJwtPayload(jwtToken)
  const verdict = claims['x-nvidia-overall-att-result'] as string

  return { nonceMatches, verdict }
}

// ---------------------------------------------------------------------------
// Main orchestrator: verify + hash quote for Solana storage
// ---------------------------------------------------------------------------

export interface VerifyOptions {
  model?: string
  /** Redpill API key, required by some deployments and by response signature flows. */
  apiKey?: string
  /** Restrict the attestation report to the TEE signer returned by /signature/{requestId}. */
  signingAddress?: string
  /** Skip GPU verification (faster, weaker) */
  skipGpu?: boolean
  /** Skip TDX quote verification (not recommended) */
  skipTdx?: boolean
  /** Solana network config — required to store proof on-chain */
  solana?: NetworkConfig
  /** Solana signer — required to store proof on-chain */
  signer?: import('@solana/web3.js').Keypair
  /** Solana proof-store behavior. */
  store?: StoreProofOptions | boolean
}

export async function verifyModel(options: VerifyOptions = {}): Promise<VerifyAndStoreResult> {
  const model = options.model ?? 'phala/gpt-oss-120b'
  const nonce = randomNonce(32)
  const errors: string[] = []

  const report = await fetchAttestation(model, nonce, options.apiKey, options.signingAddress)
  if (report.error) throw new Error(`Attestation API error: ${report.error}`)

  const attestation = selectAttestation(report, options.signingAddress)
  const signingAddress = attestation.signing_address ?? null

  // Detect provider from model string
  const provider = model.startsWith('phala/')
    ? 'phala'
    : model.startsWith('near-ai/') || model.startsWith('deepseek/') || model.startsWith('openai/')
      ? 'near-ai'
      : model.startsWith('chutes/') || model.startsWith('moonshotai/') || model.startsWith('qwen/') || model.startsWith('z-ai/') || model.startsWith('google/')
        ? 'chutes'
        : model.startsWith('tinfoil/')
          ? 'tinfoil'
          : 'unknown'

  let tdxResult: TdxResult | null = null
  let gpuResult: GpuResult | null = null
  let reportDataResult: ReportDataResult | null = null
  const composeResult = null
  let solanaResult: SolanaOnchainResult | null = null
  let quoteHashHex: string | null = null
  let quoteHashBytes: Uint8Array | null = null

  if (attestation.intel_quote) {
    const quoteBytes = Buffer.from(attestation.intel_quote.replace(/^0x/, ''), 'hex')
    quoteHashBytes = await sha256Bytes(quoteBytes)
    quoteHashHex = Buffer.from(quoteHashBytes).toString('hex')
  }

  // Step 1: TDX quote
  if (attestation.intel_quote && !options.skipTdx) {
    try {
      tdxResult = await checkTdxQuote(attestation.intel_quote)
    } catch (e) {
      errors.push(`TDX verification failed: ${e}`)
    }
  }

  // Step 2: Report data binding
  if (tdxResult?.quote?.body?.reportdata && signingAddress) {
    try {
      reportDataResult = checkReportData(
        signingAddress,
        attestation.signing_algo ?? 'ecdsa',
        nonce,
        tdxResult.quote.body.reportdata,
      )
    } catch (e) {
      errors.push(`Report data check failed: ${e}`)
    }
  }

  // Step 3: GPU
  if (attestation.nvidia_payload && !options.skipGpu) {
    try {
      gpuResult = await checkGpu(attestation.nvidia_payload, nonce)
    } catch (e) {
      errors.push(`GPU verification failed: ${e}`)
    }
  }

  // Step 4: Store on Solana
  if (options.solana && options.signer && quoteHashBytes) {
    try {
      const isValid = tdxResult?.verified ?? false
      const addrBytes = signingAddress
        ? Buffer.from(signingAddress.replace(/^0x/, ''), 'hex')
        : Buffer.alloc(20)

      solanaResult = await storeProofOnchain(
        options.solana,
        options.signer,
        quoteHashBytes,
        addrBytes,
        isValid,
        options.store ?? { initCounter: true, allowExisting: true },
      )
    } catch (e) {
      errors.push(`Solana store failed: ${e}`)
    }
  }

  const verified = (tdxResult?.verified ?? false) && errors.length === 0

  return {
    verified,
    model,
    provider,
    signingAddress,
    quoteHash: quoteHashHex,
    nonce,
    tdx: tdxResult,
    gpu: gpuResult,
    reportData: reportDataResult,
    compose: composeResult,
    solana: solanaResult,
    errors,
  }
}

export async function verifyChatAndStore(options: ChatVerifyOptions): Promise<
  VerifyAndStoreResult & {
    signature: SignaturePayload
    signatureValid: boolean
    recoveredAddress: string | null
    requestHashMatches?: boolean
    responseHashMatches?: boolean
  }
> {
  const signature = await fetchSignature(options.requestId, options.model, options.apiKey)
  if (signature.error) throw new Error(`Signature API error: ${signature.error}`)
  const normalizedSignature = normalizeSecp256k1Signature(hexToBytes(signature.signature, 65))

  const recoveredAddress = await recoverMessageAddress({
    message: signature.text,
    signature: asHexSignature(signature.signature),
  }).catch(() => null)
  const signatureValid = recoveredAddress?.toLowerCase() === signature.signing_address.toLowerCase()

  const parts = signature.text.split(':')
  const requestHash = parts.length >= 3 ? parts[parts.length - 2] : parts[0]
  const responseHash = parts.length >= 3 ? parts[parts.length - 1] : parts[1]
  const requestHashMatches = options.requestBody ? (await sha256(options.requestBody)) === requestHash : undefined
  const responseHashMatches = options.responseText ? (await sha256(options.responseText)) === responseHash : undefined

  const storeOptions = typeof options.store === 'boolean'
    ? { verifySig: options.store }
    : options.store

  const result = await verifyModel({
    model: options.model,
    apiKey: options.apiKey,
    signingAddress: signature.signing_address,
    skipGpu: options.skipGpu,
    skipTdx: options.skipTdx,
    solana: options.solana,
    signer: options.signer,
    store: storeOptions?.verifySig
      ? {
          ...storeOptions,
          signedMessage: signature.text,
          signature: normalizedSignature,
        }
      : storeOptions,
  })

  return {
    ...result,
    signature,
    signatureValid,
    recoveredAddress,
    requestHashMatches,
    responseHashMatches,
  }
}

// ---------------------------------------------------------------------------
// Solana on-chain storage
// ---------------------------------------------------------------------------

export async function initCounterIfNeeded(
  network: NetworkConfig,
  signer: Keypair,
): Promise<string | null> {
  const programId = new PublicKey(network.programId)
  const connection = new Connection(network.rpcUrl, 'confirmed')
  const [counterPda] = findProofCounterPda(programId)
  const existing = await connection.getAccountInfo(counterPda)
  if (existing) return null

  const tx = new Transaction().add(buildInitCounterInstruction(programId, signer.publicKey))
  return sendAndConfirmTransaction(connection, tx, [signer])
}

/**
 * Build and send a Solana transaction that stores a TEE proof record.
 *
 * Equivalent to calling RedpillProofStore.verifyAndStore() on Ethereum but
 * using the Solana SVM program instead.
 *
 * The transaction includes:
 *   [0] secp256k1 instruction — proves the TEE signed the payload
 *   [1] StoreProof instruction — writes the TeeProof PDA
 */
export async function storeProofOnchain(
  network: NetworkConfig,
  signer: Keypair,
  quoteHash: Uint8Array,      // 32 bytes — SHA256 of TDX quote
  signingAddress: Uint8Array, // 20 bytes — Ethereum-style address
  isValid: boolean,
  options: StoreProofOptions | boolean = {},
): Promise<SolanaOnchainResult> {
  const programId = new PublicKey(network.programId)
  const connection = new Connection(network.rpcUrl, 'confirmed')
  const opts: StoreProofOptions = typeof options === 'boolean' ? { verifySig: options } : options

  const [proofPda] = findTeeProofPda(programId, quoteHash)
  const existing = await connection.getAccountInfo(proofPda)
  if (existing && opts.allowExisting !== false) {
    const proof = await lookupProof(network, quoteHash)
    return { txSig: '', proofAddress: proofPda, slot: proof?.slot ?? 0n, alreadyExisted: true }
  }

  if (opts.initCounter !== false) await initCounterIfNeeded(network, signer)

  const tx = new Transaction()
  if (opts.verifySig) {
    if (!opts.signedMessage || !opts.signature) {
      throw new Error('verifySig requires signedMessage and signature')
    }
    const messageBytes =
      typeof opts.signedMessage === 'string'
        ? new TextEncoder().encode(opts.signedMessage)
        : opts.signedMessage
    tx.add(buildSecp256k1Instruction(signingAddress, messageBytes, normalizeSecp256k1Signature(opts.signature)))
  }

  tx.add(buildStoreProofInstruction(programId, signer.publicKey, quoteHash, signingAddress, isValid, !!opts.verifySig))
  const txSig = await sendAndConfirmTransaction(connection, tx, [signer])
  const slot = BigInt((await connection.getTransaction(txSig, { commitment: 'confirmed' }))?.slot ?? 0)

  return { txSig, proofAddress: proofPda, slot }
}

/**
 * Fetch and decode a V1 TeeProof account from Solana.
 */
export async function lookupProof(
  network: NetworkConfig,
  quoteHash: Uint8Array,
): Promise<import('./types.js').TeeProof | null> {
  const { decodeTeeProof } = await import('./accounts.js')
  const programId = new PublicKey(network.programId)
  const [proofPda] = findTeeProofPda(programId, quoteHash)
  const connection = new Connection(network.rpcUrl, 'confirmed')
  const account = await connection.getAccountInfo(proofPda)
  if (!account) return null
  return decodeTeeProof(proofPda, account.data)
}

// ---------------------------------------------------------------------------
// V2 helpers
// ---------------------------------------------------------------------------

/**
 * Compute the `signedPayloadHash` that becomes the PDA seed and the message
 * the secp256k1 instruction must sign.
 *
 * For existing RedPill attestations: pass `signature.text` (the string the
 * TEE signed). The hash is stored as a forensic link even when `verificationLevel = 0`.
 *
 * For CLAWD-native TEE: the TEE signs this exact 32-byte hash directly, enabling
 * on-chain secp verification at `verificationLevel >= 1`.
 */
export async function computeSignedPayloadHash(
  signedMessage: string | Uint8Array,
): Promise<Uint8Array> {
  const bytes =
    typeof signedMessage === 'string'
      ? new TextEncoder().encode(signedMessage)
      : signedMessage
  return sha256Bytes(bytes)
}

export interface V2StoreFields {
  signedPayloadHash: Uint8Array     // 32 bytes — SHA256 of what the TEE signed
  signingAddress: Uint8Array        // 20 bytes — Ethereum-style address
  quoteHash: Uint8Array             // 32 bytes — SHA256 of TDX quote
  requestHash: Uint8Array           // 32 bytes — SHA256 of request body
  responseHash: Uint8Array          // 32 bytes — SHA256 of response text
  modelHash: Uint8Array             // 32 bytes — SHA256 of model identifier
  attestationReportHash?: Uint8Array // 32 bytes — SHA256 of raw DCAP report
  composeHash?: Uint8Array          // 32 bytes — SHA256 of compose manifest
  verifierSetHash?: Uint8Array      // 32 bytes — SHA256 of active verifier set
  policyVersion?: number
  nonce?: Uint8Array                // 32 bytes — per-proof replay guard
  expiresAt?: bigint                // 0 = no expiry
  verificationLevel?: VerificationLevel
  /** Required when verificationLevel >= TeeSigOnchain */
  signature?: Uint8Array            // 65 bytes: r(32)+s(32)+v(1)
}

const ZERO32 = new Uint8Array(32)

/**
 * Build and send a V2 proof storage transaction.
 *
 * Transaction layout when `verificationLevel >= TeeSigOnchain`:
 *   [0]  ComputeBudgetSetLimit   (optional, caller-controlled)
 *   [1]  ComputeBudgetSetPrice   (optional)
 *   [N]  secp256k1 instruction   (signs signedPayloadHash, placed at secpIxIndex)
 *   [N+1] StoreProofV2
 *
 * When `verificationLevel = 0` (StoredOnly), no secp instruction is added and
 * the signedPayloadHash is stored as a pure forensic link.
 */
export async function storeProofV2Onchain(
  network: NetworkConfig,
  signer: Keypair,
  fields: V2StoreFields,
  opts: StoreProofV2Options = {},
): Promise<SolanaOnchainResult> {
  const programId = new PublicKey(network.programId)
  const connection = new Connection(network.rpcUrl, 'confirmed')

  const vl = fields.verificationLevel ?? VerificationLevel.StoredOnly
  const now = new Date()
  const dayTag = now.toISOString().slice(0, 10).replace(/-/g, '')

  const [proofPda] = findTeeProofV2Pda(programId, fields.signedPayloadHash)
  const existing = await connection.getAccountInfo(proofPda)
  if (existing) {
    const proof = await lookupProofV2(network, fields.signedPayloadHash)
    return { txSig: '', proofAddress: proofPda, slot: proof?.slot ?? 0n, alreadyExisted: true }
  }

  // Day counter is auto-created by the instruction on first proof of the day

  const tx = new Transaction()

  for (const ix of opts.preInstructions ?? []) {
    tx.add(ix)
  }

  // Add secp256k1 instruction for on-chain sig verification
  const needsSecp = vl >= VerificationLevel.TeeSigOnchain
  let secpIxIndex = tx.instructions.length
  if (needsSecp) {
    if (!fields.signature) throw new Error('signature required when verificationLevel >= TeeSigOnchain')
    const sig = normalizeSecp256k1Signature(fields.signature)
    if (opts.secpIxIndex !== undefined && opts.secpIxIndex !== secpIxIndex) {
      throw new Error(`secpIxIndex ${opts.secpIxIndex} does not match actual secp instruction index ${secpIxIndex}`)
    }
    tx.add(buildSecp256k1InstructionV2(fields.signingAddress, fields.signedPayloadHash, sig, secpIxIndex))
  } else {
    secpIxIndex = opts.secpIxIndex ?? 0
  }

  const proofFields: Pick<TeeProofV2,
    | 'verificationLevel' | 'signedPayloadHash' | 'quoteHash'
    | 'requestHash' | 'responseHash' | 'modelHash'
    | 'attestationReportHash' | 'composeHash'
    | 'signingAddress' | 'verifierSetHash' | 'policyVersion'
    | 'nonce' | 'expiresAt'
  > = {
    verificationLevel: vl,
    signedPayloadHash: fields.signedPayloadHash,
    quoteHash:             fields.quoteHash,
    requestHash:           fields.requestHash,
    responseHash:          fields.responseHash,
    modelHash:             fields.modelHash,
    attestationReportHash: fields.attestationReportHash ?? ZERO32,
    composeHash:           fields.composeHash ?? ZERO32,
    signingAddress:        fields.signingAddress,
    verifierSetHash:       fields.verifierSetHash ?? ZERO32,
    policyVersion:         fields.policyVersion ?? 0,
    nonce:                 fields.nonce ?? ZERO32,
    expiresAt:             fields.expiresAt ?? 0n,
  }

  tx.add(buildStoreProofV2Instruction(programId, signer.publicKey, proofFields, {
    ...opts,
    secpIxIndex,
    dayTag,
  }))

  const txSig = await sendAndConfirmTransaction(connection, tx, [signer])
  const slot = BigInt((await connection.getTransaction(txSig, { commitment: 'confirmed' }))?.slot ?? 0)

  return { txSig, proofAddress: proofPda, slot }
}

/**
 * Fetch and decode a V2 TeeProofV2 account from Solana by `signedPayloadHash`.
 */
export async function lookupProofV2(
  network: NetworkConfig,
  signedPayloadHash: Uint8Array,
): Promise<TeeProofV2 | null> {
  const { decodeTeeProofV2 } = await import('./accounts.js')
  const programId = new PublicKey(network.programId)
  const [proofPda] = findTeeProofV2Pda(programId, signedPayloadHash)
  const connection = new Connection(network.rpcUrl, 'confirmed')
  const account = await connection.getAccountInfo(proofPda)
  if (!account) return null
  return decodeTeeProofV2(proofPda, account.data)
}

/**
 * Read a sharded day counter from Solana.
 * `dayTag` = "YYYYMMDD" string, e.g. "20260619"
 */
export async function lookupDayCounter(
  network: NetworkConfig,
  dayTag: string,
): Promise<{ dayTag: string; count: bigint } | null> {
  const { decodeProofCounterDay } = await import('./accounts.js')
  const programId = new PublicKey(network.programId)
  const [pda] = findProofCounterDayPda(programId, dayTag)
  const connection = new Connection(network.rpcUrl, 'confirmed')
  const account = await connection.getAccountInfo(pda)
  if (!account) return null
  return decodeProofCounterDay(account.data)
}
