/**
 * SVM-only settlement policy.
 *
 * Solana SVM settles. TEEs compute. Digests only — never put GPU on-chain.
 * Ethereum / Sepolia / Holesky / Automata are legacy and require an explicit opt-in.
 */

import type { LegacyEvmNetworkKey, SolanaNetworkKey } from './types.js'

export const SVM_SETTLEMENT_POLICY =
  'Solana SVM settles; TEEs compute; digests only — never put GPU on-chain.'

/** Canonical default for CLI, APIs, and docs. Aliases to Solana devnet. */
export const DEFAULT_NETWORK: SolanaNetworkKey = 'solana'

export const DEFAULT_SOLANA_CLUSTER: 'devnet' | 'mainnet-beta' = 'devnet'

export const SOLANA_NETWORKS: Record<
  SolanaNetworkKey,
  { name: string; cluster: 'devnet' | 'mainnet-beta'; rpc: string; explorer: string }
> = {
  solana: {
    name: 'Solana Devnet',
    cluster: 'devnet',
    rpc: 'https://api.devnet.solana.com',
    explorer: 'https://orbmarkets.io',
  },
  devnet: {
    name: 'Solana Devnet',
    cluster: 'devnet',
    rpc: 'https://api.devnet.solana.com',
    explorer: 'https://orbmarkets.io',
  },
  'mainnet-beta': {
    name: 'Solana Mainnet',
    cluster: 'mainnet-beta',
    rpc: 'https://api.mainnet-beta.solana.com',
    explorer: 'https://orbmarkets.io',
  },
}

export const LEGACY_EVM_NETWORKS = ['automata-mainnet', 'automata-testnet', 'sepolia', 'holesky'] as const

export const CLAWD_VERIFIER_SOLANA_USAGE = `Use clawd-verifier TeeProof / TeeProofV2 (recommended):

  npx clawd-verifier lookup --quote-hash <hex> --rpc-url https://api.devnet.solana.com
  npx clawd-verifier lookup --payload-hash <hex> --rpc-url https://api.devnet.solana.com
  npx clawd-verifier anchor --keypair <path> --rpc-url https://api.devnet.solana.com`

export function isSolanaNetwork(key: string | undefined): key is SolanaNetworkKey {
  return key === 'solana' || key === 'devnet' || key === 'mainnet-beta'
}

export function isLegacyEvmNetwork(key: string | undefined): key is LegacyEvmNetworkKey {
  return (LEGACY_EVM_NETWORKS as readonly string[]).includes(key ?? '')
}

export function resolveSolanaNetwork(key?: string): (typeof SOLANA_NETWORKS)[SolanaNetworkKey] {
  const resolved = !key || key === 'solana' ? DEFAULT_NETWORK : key
  if (!isSolanaNetwork(resolved)) {
    throw new Error(
      `Unknown Solana network "${key}". Use solana, devnet, or mainnet-beta. ${SVM_SETTLEMENT_POLICY}`,
    )
  }
  return SOLANA_NETWORKS[resolved]
}

export function legacyEvmRequiredError(network: string): Error {
  return new Error(
    `Ethereum / Automata network "${network}" is deprecated and is not a production default. ` +
      `Pass --legacy-evm (or { legacyEvm: true }) to use it. ${SVM_SETTLEMENT_POLICY}\n\n` +
      CLAWD_VERIFIER_SOLANA_USAGE,
  )
}

export function assertLegacyEvmAllowed(
  network: string | undefined,
  legacyEvm: boolean | undefined,
): asserts network is LegacyEvmNetworkKey {
  if (!isLegacyEvmNetwork(network)) {
    throw new Error(
      `Expected a legacy EVM network (sepolia|holesky|automata-*), got "${network ?? ''}".`,
    )
  }
  if (!legacyEvm) throw legacyEvmRequiredError(network)
}

/** Resolve the default on-chain settlement target. Never returns an EVM network. */
export function resolveDefaultSettlement(): {
  chain: 'solana'
  network: SolanaNetworkKey
  cluster: 'devnet' | 'mainnet-beta'
  rpc: string
} {
  const net = SOLANA_NETWORKS[DEFAULT_NETWORK]
  return { chain: 'solana', network: DEFAULT_NETWORK, cluster: net.cluster, rpc: net.rpc }
}
