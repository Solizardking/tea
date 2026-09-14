import assert from 'node:assert/strict'
import { describe, it } from 'node:test'

import { CLAWD_DEFAULT_SOLANA_CLUSTER, CLAWD_DEFAULT_SOLANA_RPC } from '../constants.js'
import {
  DEFAULT_NETWORK,
  DEFAULT_SOLANA_CLUSTER,
  SOLANA_NETWORKS,
  SVM_SETTLEMENT_POLICY,
  assertLegacyEvmAllowed,
  isLegacyEvmNetwork,
  isSolanaNetwork,
  legacyEvmRequiredError,
  resolveDefaultSettlement,
  resolveSolanaNetwork,
} from './settlement.js'

describe('default settlement is Solana SVM', () => {
  it('DEFAULT_NETWORK is the solana path', () => {
    assert.equal(DEFAULT_NETWORK, 'solana')
    assert.ok(isSolanaNetwork(DEFAULT_NETWORK))
    assert.equal(isLegacyEvmNetwork(DEFAULT_NETWORK), false)
  })

  it('resolveDefaultSettlement never returns an EVM network', () => {
    const settlement = resolveDefaultSettlement()
    assert.equal(settlement.chain, 'solana')
    assert.equal(settlement.network, 'solana')
    assert.equal(settlement.cluster, DEFAULT_SOLANA_CLUSTER)
    assert.equal(settlement.cluster, 'devnet')
    assert.equal(settlement.rpc, CLAWD_DEFAULT_SOLANA_RPC)
    assert.equal(CLAWD_DEFAULT_SOLANA_CLUSTER, 'devnet')
    assert.equal(SOLANA_NETWORKS[DEFAULT_NETWORK].rpc, 'https://api.devnet.solana.com')
  })

  it('solana / devnet / mainnet-beta resolve to public Solana RPCs', () => {
    assert.equal(resolveSolanaNetwork().rpc, 'https://api.devnet.solana.com')
    assert.equal(resolveSolanaNetwork('solana').cluster, 'devnet')
    assert.equal(resolveSolanaNetwork('devnet').cluster, 'devnet')
    assert.equal(resolveSolanaNetwork('mainnet-beta').cluster, 'mainnet-beta')
  })

  it('sepolia / holesky / automata require --legacy-evm', () => {
    for (const network of ['sepolia', 'holesky', 'automata-mainnet', 'automata-testnet'] as const) {
      assert.ok(isLegacyEvmNetwork(network))
      assert.throws(() => assertLegacyEvmAllowed(network, false), /legacy-evm/)
      assert.throws(() => assertLegacyEvmAllowed(network, undefined), /deprecated/)
      assert.doesNotThrow(() => assertLegacyEvmAllowed(network, true))
    }
  })

  it('legacy EVM error names clawd-verifier Solana APIs', () => {
    const err = legacyEvmRequiredError('sepolia')
    assert.match(err.message, /clawd-verifier/)
    assert.match(err.message, /TeeProof/)
    assert.match(SVM_SETTLEMENT_POLICY, /Solana SVM settles/)
  })
})

describe('legacy EVM on-chain APIs', () => {
  it('verifyOnchain / storeProof / lookupProof throw without --legacy-evm', async () => {
    const { lookupProof, storeProof, verifyOnchain } = await import('./verifiers/onchain.js')
    const zero = '0x0000000000000000000000000000000000000000'
    await assert.rejects(() => verifyOnchain('00', 'sepolia'), /legacy-evm/)
    await assert.rejects(
      () => storeProof('00', zero as `0x${string}`, zero as `0x${string}`, `0x${'11'.repeat(32)}`, 'sepolia'),
      /legacy-evm/,
    )
    await assert.rejects(
      () => lookupProof(`0x${'11'.repeat(32)}`, zero as `0x${string}`, 'sepolia'),
      /legacy-evm/,
    )
    await assert.rejects(() => verifyOnchain('00', 'solana'), /legacy EVM network/)
  })
})
