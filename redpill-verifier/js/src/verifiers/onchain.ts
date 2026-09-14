/**
 * Legacy EVM on-chain DCAP verification via Automata smart contracts.
 *
 * Production settlement is Solana SVM (TeeProof / clawd-verifier). These
 * functions require `{ legacyEvm: true }` and never default to Sepolia.
 */

import { createPublicClient, createWalletClient, http, keccak256, type Address, type Chain, type Hex } from 'viem'
import { privateKeyToAccount } from 'viem/accounts'
import { AUTOMATA_NETWORKS, DCAP_VERIFY_ABI, PROOF_STORE_ABI } from '../constants.js'
import { assertLegacyEvmAllowed } from '../settlement.js'
import type { LegacyEvmNetworkKey, NetworkKey, OnchainVerifyResult, ProofRecord, StoreProofResult } from '../types.js'

export interface LegacyEvmCallOptions {
  /** Must be true to use Ethereum / Automata networks. */
  legacyEvm?: boolean
}

function getChain(networkKey: LegacyEvmNetworkKey): Chain {
  const net = AUTOMATA_NETWORKS[networkKey]
  return {
    id: net.chainId,
    name: net.name,
    nativeCurrency: { name: 'ETH', symbol: 'ETH', decimals: 18 },
    rpcUrls: { default: { http: [net.rpc] } },
  }
}

export async function verifyOnchain(
  intelQuote: string,
  networkKey: NetworkKey,
  options: LegacyEvmCallOptions = {},
): Promise<OnchainVerifyResult> {
  assertLegacyEvmAllowed(networkKey, options.legacyEvm)
  const network = AUTOMATA_NETWORKS[networkKey]
  const client = createPublicClient({ transport: http(network.rpc, { timeout: 120_000 }) })

  const quoteHex = `0x${intelQuote.replace(/^0x/, '')}` as Hex
  const quoteHash = keccak256(quoteHex)

  const [isValid] = await client.readContract({
    address: network.contract,
    abi: DCAP_VERIFY_ABI,
    functionName: 'verifyAndAttestOnChain',
    args: [quoteHex],
  })

  return {
    verified: isValid,
    quoteHash,
    network: network.name,
    contract: network.contract,
    explorer: `${network.explorer}/address/${network.contract}`,
  }
}

export async function storeProof(
  intelQuote: string,
  signingAddress: Address,
  proofStore: Address,
  privateKey: Hex,
  networkKey: NetworkKey,
  options: LegacyEvmCallOptions = {},
): Promise<StoreProofResult> {
  assertLegacyEvmAllowed(networkKey, options.legacyEvm)
  const network = AUTOMATA_NETWORKS[networkKey]
  const chain = getChain(networkKey)
  const quoteHex = `0x${intelQuote.replace(/^0x/, '')}` as Hex
  const quoteHash = keccak256(quoteHex)

  const account = privateKeyToAccount(privateKey)
  const walletClient = createWalletClient({ account, chain, transport: http(network.rpc, { timeout: 180_000 }) })
  const publicClient = createPublicClient({ chain, transport: http(network.rpc, { timeout: 180_000 }) })

  const txHash = await walletClient.writeContract({
    address: proofStore,
    abi: PROOF_STORE_ABI,
    functionName: 'verifyAndStore',
    args: [quoteHex, signingAddress],
  })

  const receipt = await publicClient.waitForTransactionReceipt({ hash: txHash, timeout: 300_000 })

  return {
    txHash,
    quoteHash,
    isValid: receipt.status === 'success',
    blockNumber: receipt.blockNumber,
    gasUsed: receipt.gasUsed,
    explorer: `${network.explorer}/tx/${txHash}`,
  }
}

export async function lookupProof(
  quoteHash: Hex,
  proofStore: Address,
  networkKey: NetworkKey,
  options: LegacyEvmCallOptions = {},
): Promise<ProofRecord | null> {
  assertLegacyEvmAllowed(networkKey, options.legacyEvm)
  const network = AUTOMATA_NETWORKS[networkKey]
  const client = createPublicClient({ transport: http(network.rpc, { timeout: 30_000 }) })

  const proof = await client.readContract({
    address: proofStore,
    abi: PROOF_STORE_ABI,
    functionName: 'getProof',
    args: [quoteHash],
  })

  const p = proof as unknown as Record<string, unknown>
  const qh = (p.quoteHash ?? p[0]) as Hex
  if (qh === '0x0000000000000000000000000000000000000000000000000000000000000000') return null

  return {
    quoteHash: qh,
    signingAddress: (p.signingAddress ?? p[1]) as Address,
    isValid: (p.isValid ?? p[2]) as boolean,
    timestamp: (p.timestamp ?? p[3]) as bigint,
    blockNumber: (p.blockNumber ?? p[4]) as bigint,
    submitter: (p.submitter ?? p[5]) as Address,
  }
}
