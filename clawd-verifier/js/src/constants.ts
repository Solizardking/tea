/**
 * CLAWD deployment defaults.
 *
 * Settlement is Solana SVM only (devnet by default; override to mainnet-beta).
 * Ethereum / Sepolia / Holesky / Automata are not production defaults.
 */

export const CLAWD_SOLANA_PROGRAM_ID = 'HvXe5RpCcduVYJWDyavmNKde4uEgUFEiYz99JYQT4DkP'
export const CLAWD_DEFAULT_SOLANA_CLUSTER = 'devnet' as const
export const CLAWD_DEFAULT_SOLANA_RPC = 'https://api.devnet.solana.com'
export const CLAWD_LOCAL_SOLANA_RPC = 'http://127.0.0.1:8899'
/** Local compose / cargo run. Hosted Railway: https://clawd-tee-gateway-production.up.railway.app */
export const CLAWD_GATEWAY_URL = 'http://127.0.0.1:8081'
export const CLAWD_HOSTED_GATEWAY_URL =
  'https://clawd-tee-gateway-production.up.railway.app'
