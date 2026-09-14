/**
 * Kora JSON-RPC client used by the x402 facilitator.
 *
 * Method names and shapes match `@solana/kora` `KoraClient`
 * (`signTransaction`, `signAndSendTransaction`, `getPayerSigner`, `getConfig`).
 */
import { KoraClient } from "@solana/kora";

export interface KoraClientOptions {
	rpcUrl: string;
	apiKey?: string;
}

export interface SignTransactionRequest {
	transaction: string;
	signer_key?: string;
}

export interface SignTransactionResponse {
	signature?: string;
	signed_transaction: string;
	signer_pubkey?: string;
}

export interface SignAndSendTransactionRequest {
	transaction: string;
	signer_key?: string;
}

export interface SignAndSendTransactionResponse {
	signature: string;
	signed_transaction?: string;
}

export interface GetPayerSignerResponse {
	signer_address: string;
	payment_address?: string;
}

export type ClawdKoraClient = Pick<
	KoraClient,
	"signTransaction" | "signAndSendTransaction" | "getPayerSigner" | "getConfig"
>;

export function createKoraClient(options: KoraClientOptions): ClawdKoraClient {
	return new KoraClient({
		rpcUrl: options.rpcUrl,
		apiKey: options.apiKey,
	});
}
