/**
 * @clawd/tee-verifier — one package, the whole trust chain.
 *
 * Combines:
 *   - the multi-provider TEE verification engine (Phala / NearAI / Chutes /
 *     Tinfoil; light + deep modes; Automata DCAP; Intel Trust Authority)
 *   - the Solana on-chain proof client (TeeProof V1/V2 PDAs, secp256k1
 *     precompile verification, day counters)
 *   - Legacy EVM proof storage (ClawdProofStore + Automata DCAP; --legacy-evm only)
 *
 * @example One call: verify a TEE model and anchor the proof on Solana
 * ```typescript
 * import { verifyAndAnchor } from '@clawd/tee-verifier'
 *
 * const result = await verifyAndAnchor({
 *   model: 'phala/gpt-oss-120b',
 *   solana: { rpcUrl, programId, signer: keypair },
 * })
 * result.verified                    // true
 * result.solana?.txSig               // Solana tx anchoring the proof
 * ```
 */

// Unified pipeline
export { verifyAndAnchor } from './anchor.js'
export type {
  VerifyAndAnchorOptions,
  VerifyAndAnchorResult,
  SolanaAnchorConfig,
  SolanaAnchorResult,
  EvmAnchorConfig,
  ChatBinding,
} from './anchor.js'

// Top-level convenience: the core verification API
export { verify, verifyModel } from './core/verify.js'
export type { VerifyOptions, VerifyResult, VerifyModelOptions, VerifyModelResult } from './core/verify.js'

// Full engines, namespaced (their internal names overlap — e.g. both expose
// their own verifyModel/lookupProof — so they are not flattened)
export * as tee from './core/index.js'
export * as solana from './solana/index.js'

// CLAWD deployment defaults (Solana SVM)
export {
  CLAWD_SOLANA_PROGRAM_ID,
  CLAWD_DEFAULT_SOLANA_CLUSTER,
  CLAWD_DEFAULT_SOLANA_RPC,
  CLAWD_LOCAL_SOLANA_RPC,
  CLAWD_GATEWAY_URL,
  CLAWD_HOSTED_GATEWAY_URL,
} from './constants.js'
export {
  DEFAULT_NETWORK,
  DEFAULT_SOLANA_CLUSTER,
  SOLANA_NETWORKS,
  SVM_SETTLEMENT_POLICY,
  isSolanaNetwork,
  resolveDefaultSettlement,
  resolveSolanaNetwork,
} from './core/settlement.js'
