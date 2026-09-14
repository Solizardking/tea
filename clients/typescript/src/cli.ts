#!/usr/bin/env node

import { existsSync, readFileSync } from 'node:fs'
import { homedir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { Connection, Keypair, PublicKey } from '@solana/web3.js'
import {
  computeSignedPayloadHash,
  decodeProofCounter,
  findProofCounterPda,
  hexToBytes,
  initCounterIfNeeded,
  lookupDayCounter,
  lookupProof,
  lookupProofV2,
  storeProofOnchain,
  verifyChatAndStore,
  verifyModel,
} from './index.js'
import { VerificationLevel } from './types.js'
import type { NetworkConfig } from './types.js'
import type { VerifyAndStoreResult } from './types.js'

type Args = {
  command: string
  flags: Map<string, string | true>
  positional: string[]
}

function loadDotEnvUpwards(start = process.cwd()): void {
  let dir = resolve(start)
  for (let depth = 0; depth < 8; depth += 1) {
    const envPath = join(dir, '.env')
    if (existsSync(envPath)) loadDotEnv(envPath)
    const parent = dirname(dir)
    if (parent === dir) break
    dir = parent
  }
}

function loadDotEnv(path: string): void {
  const lines = readFileSync(path, 'utf8').split(/\r?\n/)
  for (const raw of lines) {
    const line = raw.trim()
    if (!line || line.startsWith('#')) continue
    const match = line.match(/^(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=(.*)$/)
    if (!match) continue

    const key = match[1]
    if (process.env[key] !== undefined) continue

    let value = match[2].trim()
    if ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'"))) {
      value = value.slice(1, -1)
    }
    process.env[key] = value
  }
}

function parseArgs(argv: string[]): Args {
  const [command = 'help', ...rest] = argv
  const flags = new Map<string, string | true>()
  const positional: string[] = []

  for (let i = 0; i < rest.length; i += 1) {
    const token = rest[i]
    if (!token.startsWith('--')) {
      positional.push(token)
      continue
    }

    const eq = token.indexOf('=')
    if (eq !== -1) {
      flags.set(token.slice(2, eq), token.slice(eq + 1))
      continue
    }

    const name = token.slice(2)
    const next = rest[i + 1]
    if (next && !next.startsWith('--')) {
      flags.set(name, next)
      i += 1
    } else {
      flags.set(name, true)
    }
  }

  return { command, flags, positional }
}

function flag(args: Args, name: string, fallback?: string): string | undefined {
  const value = args.flags.get(name)
  if (value === undefined || value === true) return fallback
  return value
}

function hasFlag(args: Args, name: string): boolean {
  return args.flags.has(name)
}

function requireEnv(name: string): string {
  const value = process.env[name]
  if (!value) throw new Error(`Missing ${name}`)
  return value
}

function expandHome(path: string): string {
  return path.startsWith('~/') ? join(homedir(), path.slice(2)) : path
}

function loadSigner(args: Args): Keypair {
  const keypairPath = expandHome(
    flag(args, 'keypair')
      ?? process.env.SOLANA_KEYPAIR_PATH
      ?? join(homedir(), '.config/solana/id.json'),
  )
  const secret = JSON.parse(readFileSync(keypairPath, 'utf8')) as number[]
  return Keypair.fromSecretKey(Uint8Array.from(secret))
}

function networkFromEnv(args: Args): NetworkConfig {
  const programId =
    flag(args, 'program-id')
    ?? process.env.REDPILL_PROGRAM_ID
    ?? process.env.SOLANA_REDPILL_PROGRAM_ID
    ?? process.env.NEXT_PUBLIC_REDPILL_PROGRAM_ID

  if (!programId) {
    throw new Error('Missing REDPILL_PROGRAM_ID or --program-id')
  }

  return {
    rpcUrl:
      flag(args, 'rpc-url')
      ?? process.env.SOLANA_RPC_URL
      ?? process.env.NEXT_PUBLIC_SOLANA_RPC_URL
      ?? 'https://api.devnet.solana.com',
    programId,
  }
}

function toJson(value: unknown): string {
  return JSON.stringify(
    value,
    (_key, item) => {
      if (typeof item === 'bigint') return item.toString()
      if (item instanceof PublicKey) return item.toBase58()
      if (item instanceof Uint8Array) return Buffer.from(item).toString('hex')
      return item
    },
    2,
  )
}

function usage(): string {
  return `solana-redpill-verifier

V1 commands:
  init-counter                         Create the global ProofCounter PDA (one-time setup)
  counter                              Read the global ProofCounter PDA
  store-raw --quote-hash <hex> --signing-address <hex>
                                       Store a V1 proof record explicitly (smoke tests / recovery)
  store-model --model <model>          Verify a model TEE quote and store V1 proof on Solana
  store-chat --request-id <id> --model <model>
                                       Verify a RedPill response signature and store V1 proof
  lookup --quote-hash <hex>            Read a V1 TeeProof PDA by SHA256(quote)

V2 commands:
  lookup-v2 --payload-hash <hex>       Read a V2 TeeProofV2 PDA by signedPayloadHash
  day-counter --day <YYYYMMDD>         Read a sharded day counter (defaults to today)
  hash-message --message <text>        Print SHA256 of a message (the signedPayloadHash for V2)

Common flags:
  --program-id <pubkey>                Defaults to REDPILL_PROGRAM_ID or SOLANA_REDPILL_PROGRAM_ID
  --rpc-url <url>                      Defaults to SOLANA_RPC_URL, then devnet
  --keypair <path>                     Defaults to SOLANA_KEYPAIR_PATH, then ~/.config/solana/id.json
  --skip-gpu                           Skip NVIDIA NRAS verification
  --skip-tdx                           Skip Phala TDX quote verification
  --summary                            Print compact verifier output
`
}

function summarizeVerifyResult(result: VerifyAndStoreResult): Record<string, unknown> {
  return {
    verified: result.verified,
    model: result.model,
    provider: result.provider,
    signingAddress: result.signingAddress,
    quoteHash: result.quoteHash,
    nonce: result.nonce,
    tdxVerified: result.tdx?.verified ?? null,
    gpu: result.gpu,
    reportData: result.reportData,
    solana: result.solana,
    errors: result.errors,
  }
}

async function main(): Promise<void> {
  loadDotEnvUpwards()
  const args = parseArgs(process.argv.slice(2))

  if (args.command === 'help' || hasFlag(args, 'help')) {
    console.log(usage())
    return
  }

  const network = networkFromEnv(args)

  if (args.command === 'init-counter') {
    const signer = loadSigner(args)
    const txSig = await initCounterIfNeeded(network, signer)
    console.log(toJson({ ok: true, alreadyInitialized: txSig === null, txSig }))
    return
  }

  if (args.command === 'counter') {
    const programId = new PublicKey(network.programId)
    const [counterPda] = findProofCounterPda(programId)
    const account = await new Connection(network.rpcUrl, 'confirmed').getAccountInfo(counterPda)
    console.log(toJson({
      ok: true,
      address: counterPda,
      counter: account ? decodeProofCounter(account.data) : null,
    }))
    return
  }

  if (args.command === 'store-raw') {
    const signer = loadSigner(args)
    const quoteHash = flag(args, 'quote-hash', args.positional[0])
    const signingAddress = flag(args, 'signing-address', args.positional[1])
    if (!quoteHash) throw new Error('Missing --quote-hash')
    if (!signingAddress) throw new Error('Missing --signing-address')

    const result = await storeProofOnchain(
      network,
      signer,
      hexToBytes(quoteHash, 32),
      hexToBytes(signingAddress, 20),
      !hasFlag(args, 'invalid'),
      { initCounter: true, allowExisting: true },
    )
    console.log(toJson({ ok: true, result }))
    return
  }

  if (args.command === 'store-model') {
    const signer = loadSigner(args)
    const model = flag(args, 'model', args.positional[0] ?? 'phala/gpt-oss-120b')
    const result = await verifyModel({
      model,
      apiKey: process.env.REDPILL_API_KEY,
      skipGpu: hasFlag(args, 'skip-gpu'),
      skipTdx: hasFlag(args, 'skip-tdx'),
      solana: network,
      signer,
      store: { initCounter: true, allowExisting: true },
    })
    console.log(toJson({ ok: result.verified, result: hasFlag(args, 'summary') ? summarizeVerifyResult(result) : result }))
    return
  }

  if (args.command === 'store-chat') {
    const signer = loadSigner(args)
    const model = flag(args, 'model', args.positional[1])
    const requestId = flag(args, 'request-id', args.positional[0])
    if (!model) throw new Error('Missing --model')
    if (!requestId) throw new Error('Missing --request-id')

    const result = await verifyChatAndStore({
      requestId,
      model,
      apiKey: requireEnv('REDPILL_API_KEY'),
      skipGpu: hasFlag(args, 'skip-gpu'),
      skipTdx: hasFlag(args, 'skip-tdx'),
      solana: network,
      signer,
      store: { verifySig: !hasFlag(args, 'no-verify-sig'), initCounter: true, allowExisting: true },
    })
    console.log(toJson({
      ok: result.verified && result.signatureValid,
      result: hasFlag(args, 'summary')
        ? {
            ...summarizeVerifyResult(result),
            signatureValid: result.signatureValid,
            recoveredAddress: result.recoveredAddress,
            requestHashMatches: result.requestHashMatches,
            responseHashMatches: result.responseHashMatches,
          }
        : result,
    }))
    return
  }

  if (args.command === 'lookup') {
    const quoteHash = flag(args, 'quote-hash', args.positional[0])
    if (!quoteHash) throw new Error('Missing --quote-hash')
    const proof = await lookupProof(network, hexToBytes(quoteHash, 32))
    console.log(toJson({ ok: true, proof }))
    return
  }

  // ── V2 commands ───────────────────────────────────────────────────────────

  if (args.command === 'lookup-v2') {
    const payloadHash = flag(args, 'payload-hash', args.positional[0])
    if (!payloadHash) throw new Error('Missing --payload-hash')
    const proof = await lookupProofV2(network, hexToBytes(payloadHash, 32))
    console.log(toJson({ ok: true, proof }))
    return
  }

  if (args.command === 'day-counter') {
    const day = flag(args, 'day', args.positional[0])
      ?? new Date().toISOString().slice(0, 10).replace(/-/g, '')
    const counter = await lookupDayCounter(network, day)
    console.log(toJson({ ok: true, day, counter }))
    return
  }

  if (args.command === 'hash-message') {
    const message = flag(args, 'message', args.positional[0])
    if (!message) throw new Error('Missing --message')
    const hash = await computeSignedPayloadHash(message)
    console.log(toJson({
      ok: true,
      message,
      signedPayloadHash: Buffer.from(hash).toString('hex'),
      verificationLevel: VerificationLevel.TeeSigOnchain,
      note: 'Pass signedPayloadHash as --payload-hash to lookup-v2 after storing',
    }))
    return
  }

  throw new Error(`Unknown command: ${args.command}`)
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error)
  console.error(toJson({ ok: false, error: message }))
  process.exit(1)
})
