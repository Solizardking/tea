/**
 * clawd-verifier — unified CLI.
 *
 *   attest         verify a model's TEE attestation (light or deep)
 *   verify         verify an existing chat response (signature + attestation)
 *   anchor         verify then anchor the proof on Solana (and optionally EVM)
 *   lookup         look up an anchored proof on Solana (v1 quote hash or v2 payload hash)
 *   lookup-evm     look up a stored proof on an EVM ClawdProofStore
 *   health         check the clawd-tee-gateway health endpoint
 */

import { readFileSync } from 'node:fs'
import { Keypair, PublicKey } from '@solana/web3.js'
import type { Address, Hex } from 'viem'

import { verify, verifyModel } from '../src/core/verify.js'
import { lookupProof as lookupProofEvm } from '../src/core/verifiers/onchain.js'
import type { NetworkKey } from '../src/core/types.js'
import { verifyAndAnchor } from '../src/anchor.js'
import { hexToBytes, lookupProof, lookupProofV2 } from '../src/solana/verify.js'
import { CLAWD_DEFAULT_SOLANA_RPC, CLAWD_GATEWAY_URL, CLAWD_SOLANA_PROGRAM_ID } from '../src/constants.js'
import { DEFAULT_NETWORK, SVM_SETTLEMENT_POLICY, isLegacyEvmNetwork } from '../src/core/settlement.js'

function parseArgs(argv: string[]): { positional: string[]; flags: Record<string, string | boolean> } {
  const positional: string[] = []
  const flags: Record<string, string | boolean> = {}
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i]
    if (a.startsWith('--')) {
      const key = a.slice(2)
      const next = argv[i + 1]
      if (next !== undefined && !next.startsWith('--')) {
        flags[key] = next
        i++
      } else {
        flags[key] = true
      }
    } else {
      positional.push(a)
    }
  }
  return { positional, flags }
}

function str(v: string | boolean | undefined): string | undefined {
  return typeof v === 'string' ? v : undefined
}

function jsonReplacer(_key: string, value: unknown): unknown {
  if (typeof value === 'bigint') return value.toString()
  if (value instanceof Uint8Array) return Buffer.from(value).toString('hex')
  if (value instanceof PublicKey) return value.toBase58()
  return value
}

function print(result: unknown): void {
  console.log(JSON.stringify(result, jsonReplacer, 2))
}

function loadKeypair(path: string): Keypair {
  const raw = JSON.parse(readFileSync(path, 'utf-8')) as number[]
  return Keypair.fromSecretKey(Uint8Array.from(raw))
}

function usage(): void {
  console.log(`clawd-verifier — TEE verification + on-chain proof anchoring

${SVM_SETTLEMENT_POLICY}

Usage:
  clawd-verifier attest --model <model> [--deep|--light] [--ita-key <key>] [--network ${DEFAULT_NETWORK}|devnet|mainnet-beta]
  clawd-verifier verify <chatId> --api-key <key> [--model <model>]
  clawd-verifier anchor --keypair <path> [--model <model>] [--rpc-url <url>] [--program-id <id>]
                        [--request-id <chatId> --api-key <key>]
                        [--legacy-evm --evm-private-key <0x..> --evm-proof-store <0x..> --evm-network <net>]
  clawd-verifier lookup (--quote-hash <hex> | --payload-hash <hex>) [--rpc-url <url>] [--program-id <id>]
  clawd-verifier lookup-evm --legacy-evm --quote-hash <0x..> --proof-store <0x..> [--network <net>]
  clawd-verifier health [--gateway-url <url>]
                 hosted: https://clawd-tee-gateway-production.up.railway.app

Defaults:
  --network      ${DEFAULT_NETWORK} (Solana ${CLAWD_DEFAULT_SOLANA_RPC.includes('devnet') ? 'devnet' : 'cluster'})
  --rpc-url      ${CLAWD_DEFAULT_SOLANA_RPC}
  --program-id   ${CLAWD_SOLANA_PROGRAM_ID}
  --gateway-url  ${CLAWD_GATEWAY_URL} (local) or https://clawd-tee-gateway-production.up.railway.app
`)
}

async function main(): Promise<void> {
  const [command, ...rest] = process.argv.slice(2)
  const { positional, flags } = parseArgs(rest)

  const rpcUrl = str(flags['rpc-url']) ?? CLAWD_DEFAULT_SOLANA_RPC
  const programId = str(flags['program-id']) ?? CLAWD_SOLANA_PROGRAM_ID

  switch (command) {
    case 'attest': {
      const result = await verifyModel({
        model: str(flags.model),
        deep: flags.deep === true ? true : flags.light === true ? false : undefined,
        network: (str(flags.network) as NetworkKey | undefined) ?? DEFAULT_NETWORK,
        legacyEvm: flags['legacy-evm'] === true,
        skipOnchain: flags['skip-onchain'] === true || flags['legacy-evm'] !== true,
        itaApiKey: str(flags['ita-key']),
      })
      print(result)
      process.exitCode = result.verified ? 0 : 1
      break
    }

    case 'verify': {
      const chatId = positional[0]
      const apiKey = str(flags['api-key'])
      if (!chatId || !apiKey) throw new Error('verify requires <chatId> and --api-key')
      const result = await verify(chatId, { apiKey, model: str(flags.model) })
      print(result)
      process.exitCode = result.verified ? 0 : 1
      break
    }

    case 'anchor': {
      const keypairPath = str(flags.keypair)
      if (!keypairPath) throw new Error('anchor requires --keypair <path>')
      const signer = loadKeypair(keypairPath)

      const requestId = str(flags['request-id'])
      const apiKey = str(flags['api-key'])
      const evmKey = str(flags['evm-private-key'])
      const evmStore = str(flags['evm-proof-store'])
      if ((evmKey || evmStore) && flags['legacy-evm'] !== true) {
        throw new Error(
          `EVM anchoring is deprecated. Pass --legacy-evm to write ClawdProofStore, or omit --evm-* and use Solana TeeProof. ${SVM_SETTLEMENT_POLICY}`,
        )
      }

      const result = await verifyAndAnchor({
        model: str(flags.model),
        deep: flags.deep === true ? true : flags.light === true ? false : undefined,
        skipOnchain: flags['skip-onchain'] === true,
        chat: requestId && apiKey ? { requestId, apiKey } : undefined,
        solana: { rpcUrl, programId, signer },
        evm: evmKey && evmStore
          ? {
              privateKey: evmKey as Hex,
              proofStore: evmStore as Address,
              network: str(flags['evm-network']) as NetworkKey | undefined,
              legacyEvm: flags['legacy-evm'] === true,
            }
          : undefined,
      })
      print(result)
      process.exitCode = result.verified && result.solana !== null ? 0 : 1
      break
    }

    case 'lookup': {
      const quoteHash = str(flags['quote-hash'])
      const payloadHash = str(flags['payload-hash'])
      if (!quoteHash && !payloadHash) throw new Error('lookup requires --quote-hash or --payload-hash')
      const network = { rpcUrl, programId }
      const result = quoteHash
        ? await lookupProof(network, hexToBytes(quoteHash, 32))
        : await lookupProofV2(network, hexToBytes(payloadHash!, 32))
      if (!result) {
        console.error('No proof found')
        process.exitCode = 1
        break
      }
      print(result)
      break
    }

    case 'lookup-evm': {
      const quoteHash = str(flags['quote-hash'])
      const proofStore = str(flags['proof-store'])
      if (!quoteHash || !proofStore) throw new Error('lookup-evm requires --quote-hash and --proof-store')
      const evmNetwork = (str(flags.network) as NetworkKey | undefined) ?? 'sepolia'
      if (flags['legacy-evm'] !== true || !isLegacyEvmNetwork(evmNetwork)) {
        throw new Error(
          `lookup-evm is a deprecated Ethereum path. Pass --legacy-evm --network sepolia|holesky|automata-*. ${SVM_SETTLEMENT_POLICY}`,
        )
      }
      const result = await lookupProofEvm(
        (quoteHash.startsWith('0x') ? quoteHash : `0x${quoteHash}`) as Hex,
        proofStore as Address,
        evmNetwork,
        { legacyEvm: true },
      )
      if (!result) {
        console.error('No proof found')
        process.exitCode = 1
        break
      }
      print(result)
      break
    }

    case 'health': {
      const url = str(flags['gateway-url']) ?? CLAWD_GATEWAY_URL
      const res = await fetch(`${url.replace(/\/$/, '')}/health`, { signal: AbortSignal.timeout(10_000) })
      const body = await res.json()
      print({ status: res.status, ...body as Record<string, unknown> })
      process.exitCode = res.ok ? 0 : 1
      break
    }

    case 'help':
    case undefined:
      usage()
      break

    default:
      console.error(`Unknown command: ${command}\n`)
      usage()
      process.exitCode = 1
  }
}

main().catch((e) => {
  console.error(e instanceof Error ? e.message : e)
  process.exitCode = 1
})
