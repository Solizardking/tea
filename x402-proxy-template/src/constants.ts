/** $CLAWD pump mint used as the x402 payment asset and Kora fee token */
export const CLAWD_MINT = "8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump";

export const SOLANA_NETWORKS = new Set(["solana-devnet", "solana"]);

export function isSolanaNetwork(network: string | undefined): boolean {
	if (!network) return false;
	return SOLANA_NETWORKS.has(network) || network.startsWith("solana:");
}
