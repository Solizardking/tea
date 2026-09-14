/** $CLAWD pump mint — must match clawd-kora/kora.toml allowed_spl_paid_tokens */
export const CLAWD_MINT = "8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump";

/** x402 v1 SVM network id used by x402-hono / x402 1.x clients */
export const SOLANA_DEVNET = "solana-devnet";

/** Solana mainnet x402 v1 id */
export const SOLANA_MAINNET = "solana";

/** Devnet genesis CAIP-2 (x402 v2 / Kora demo) */
export const SOLANA_DEVNET_CAIP2 = "solana:EtWTRABZaYq6iMfeYKouRu166VU2xqa1";

export const DEFAULT_KORA_RPC_URL = "http://127.0.0.1:8080";
export const DEFAULT_FACILITATOR_PORT = 3000;
export const DEFAULT_NETWORK = SOLANA_DEVNET;

export function isSolanaNetwork(network: unknown): network is string {
	if (typeof network !== "string" || network.length === 0) {
		return false;
	}
	return (
		network === SOLANA_DEVNET ||
		network === SOLANA_MAINNET ||
		network.startsWith("solana:")
	);
}
