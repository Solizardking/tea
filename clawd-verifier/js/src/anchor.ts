/**
 * verifyAndAnchor — the CLAWD unified pipeline.
 *
 * 1. Deep multi-provider TEE verification (Phala / NearAI / Chutes / Tinfoil)
 *    via the core engine: TDX quote, report_data binding, GPU (NRAS), compose,
 *    Sigstore, optional dstack deep mode, optional Automata on-chain DCAP,
 *    optional Intel Trust Authority.
 * 2. Optional per-response signature check (ecrecover over the TEE-signed text).
 * 3. Anchoring:
 *    - Solana: TeeProof V1 (quote hash) or TeeProofV2 (full hash set + forensic
 *      signedPayloadHash) on the CLAWD verifier program.
 *    - EVM: Automata DCAP verifyAndStore on ClawdProofStore.
 */

import type { Keypair } from '@solana/web3.js'
import { recoverMessageAddress, type Address, type Hex } from 'viem'

import { verifyModel, type VerifyModelOptions, type VerifyModelResult } from './core/verify.js'
import { storeProof as storeProofEvm } from './core/verifiers/onchain.js'
import type { NetworkKey, StoreProofResult } from './core/types.js'

import {
  computeSignedPayloadHash,
  fetchSignature,
  hexToBytes,
  sha256Bytes,
  storeProofOnchain,
  storeProofV2Onchain,
} from './solana/verify.js'
import { VerificationLevel } from './solana/types.js'
import type { NetworkConfig, SignaturePayload, SolanaOnchainResult } from './solana/types.js'

export interface SolanaAnchorConfig extends NetworkConfig {
  signer: Keypair
}

export interface EvmAnchorConfig {
  /** Legacy EVM network. Requires `legacyEvm: true`. */
  network?: NetworkKey
  proofStore: Address
  privateKey: Hex
  /** Must be true to write to Ethereum / Automata. */
  legacyEvm?: boolean
}

/** Bind a specific chat response (request/response hashes + TEE signature). */
export interface ChatBinding {
  requestId: string
  apiKey: string
  requestBody?: string
  responseText?: string
}

export interface VerifyAndAnchorOptions extends VerifyModelOptions {
  chat?: ChatBinding
  solana?: SolanaAnchorConfig
  evm?: EvmAnchorConfig
}

export interface SolanaAnchorResult extends SolanaOnchainResult {
  kind: 'v1' | 'v2'
  /** hex SHA256 of the raw Intel quote bytes */
  quoteHash?: string
  /** hex SHA256 of the TEE-signed text (V2 PDA seed) */
  signedPayloadHash?: string
}

export interface VerifyAndAnchorResult {
  /** verification.verified && signature.valid (when a chat binding was given) */
  verified: boolean
  verification: VerifyModelResult
  signature: {
    valid: boolean
    recoveredAddress: string | null
    signingAddress: string
    text: string
  } | null
  solana: SolanaAnchorResult | null
  evm: StoreProofResult | null
  errors: string[]
}

function bytesToHex(bytes: Uint8Array): string {
  return Array.from(bytes)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
}

export async function verifyAndAnchor(
  options: VerifyAndAnchorOptions = {},
): Promise<VerifyAndAnchorResult> {
  const errors: string[] = []

  // 1. Full TEE verification via the core engine
  const verification = await verifyModel(options)

  // 2. Optional per-response signature (ecrecover)
  let sig: SignaturePayload | null = null
  let signature: VerifyAndAnchorResult['signature'] = null
  if (options.chat) {
    try {
      sig = await fetchSignature(options.chat.requestId, verification.model, options.chat.apiKey)
      if (sig.error) throw new Error(sig.error)
      const sigHex = (sig.signature.startsWith('0x') ? sig.signature : `0x${sig.signature}`) as Hex
      const recovered = await recoverMessageAddress({ message: sig.text, signature: sigHex }).catch(() => null)
      signature = {
        valid: recovered?.toLowerCase() === sig.signing_address.toLowerCase(),
        recoveredAddress: recovered,
        signingAddress: sig.signing_address,
        text: sig.text,
      }
    } catch (e) {
      sig = null
      errors.push(`Signature fetch/verify failed: ${e}`)
    }
  }

  const quoteBytes = verification.intelQuote ? hexToBytes(verification.intelQuote) : null
  const quoteHash = quoteBytes ? await sha256Bytes(quoteBytes) : null

  // 3a. Solana anchoring
  let solana: SolanaAnchorResult | null = null
  if (options.solana) {
    const { signer, rpcUrl, programId } = options.solana
    const network: NetworkConfig = { rpcUrl, programId }
    try {
      if (sig && signature) {
        // V2 record: full hash set, signedPayloadHash as forensic link (level 0).
        // Level 1 (on-chain secp) is reserved for CLAWD-native TEEs that sign
        // the 32-byte payload hash directly (see clawd-tee-gateway).
        const signedPayloadHash = await computeSignedPayloadHash(sig.text)
        const parts = sig.text.split(':')
        const reqHex = parts.length >= 3 ? parts[parts.length - 2] : parts[0]
        const respHex = parts.length >= 3 ? parts[parts.length - 1] : parts[1]
        const result = await storeProofV2Onchain(network, signer, {
          signedPayloadHash,
          signingAddress: hexToBytes(sig.signing_address, 20),
          quoteHash: quoteHash ?? new Uint8Array(32),
          requestHash: hexToBytes(reqHex, 32),
          responseHash: hexToBytes(respHex, 32),
          modelHash: await sha256Bytes(new TextEncoder().encode(verification.model)),
          verificationLevel: VerificationLevel.StoredOnly,
        })
        solana = {
          ...result,
          kind: 'v2',
          signedPayloadHash: bytesToHex(signedPayloadHash),
          quoteHash: quoteHash ? bytesToHex(quoteHash) : undefined,
        }
      } else if (quoteHash) {
        // V1 record: quote hash + signing address + off-chain verdict.
        const addr = verification.signingAddress
          ? hexToBytes(verification.signingAddress, 20)
          : new Uint8Array(20)
        const result = await storeProofOnchain(network, signer, quoteHash, addr, verification.verified)
        solana = { ...result, kind: 'v1', quoteHash: bytesToHex(quoteHash) }
      } else {
        errors.push('Solana anchoring skipped: attestation returned no Intel quote')
      }
    } catch (e) {
      errors.push(`Solana anchoring failed: ${e}`)
    }
  }

  // 3b. EVM anchoring (Automata DCAP verifyAndStore on ClawdProofStore)
  let evm: StoreProofResult | null = null
  if (options.evm) {
    try {
      if (!verification.intelQuote) throw new Error('attestation returned no Intel quote')
      const signingAddress = (verification.signingAddress ?? '0x0000000000000000000000000000000000000000') as Address
      evm = await storeProofEvm(
        verification.intelQuote,
        signingAddress,
        options.evm.proofStore,
        options.evm.privateKey,
        options.evm.network ?? 'sepolia',
        { legacyEvm: options.evm.legacyEvm },
      )
    } catch (e) {
      errors.push(`EVM anchoring failed: ${e}`)
    }
  }

  return {
    verified: verification.verified && (signature ? signature.valid : true),
    verification,
    signature,
    solana,
    evm,
    errors: [...verification.errors, ...errors],
  }
}
