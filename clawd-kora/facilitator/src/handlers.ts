import { createKoraClient, type ClawdKoraClient } from "./kora.ts";
import { DEFAULT_NETWORK, isSolanaNetwork } from "./constants.ts";

export interface FacilitatorConfig {
	koraRpcUrl: string;
	koraApiKey?: string;
	network: string;
	kora?: ClawdKoraClient;
}

export interface PaymentPayload {
	x402Version?: number;
	scheme?: string;
	network?: string;
	payload?: {
		transaction?: string;
		[key: string]: unknown;
	};
	[key: string]: unknown;
}

export interface PaymentRequirements {
	scheme?: string;
	network?: string;
	[key: string]: unknown;
}

export interface VerifyRequestBody {
	x402Version?: number;
	paymentPayload?: PaymentPayload;
	paymentRequirements?: PaymentRequirements;
}

export interface VerifyResponse {
	isValid: boolean;
	invalidReason?: string;
	payer?: string;
}

export interface SettleResponse {
	success: boolean;
	transaction: string;
	network: string;
	errorReason?: string;
}

export interface SupportedKind {
	x402Version: number;
	scheme: "exact";
	network: string;
	extra: { feePayer: string };
}

export interface SupportedResponse {
	kinds: SupportedKind[];
}

function getKora(config: FacilitatorConfig): ClawdKoraClient {
	return (
		config.kora ??
		createKoraClient({
			rpcUrl: config.koraRpcUrl,
			apiKey: config.koraApiKey,
		})
	);
}

/**
 * Pull the base64 Solana transaction out of an x402 exact/SVM payment payload.
 */
export function extractSolanaTransaction(
	paymentPayload: PaymentPayload | undefined
): string {
	const nested = paymentPayload?.payload;
	const direct =
		nested && typeof nested === "object"
			? (nested as { transaction?: unknown }).transaction
			: undefined;
	if (typeof direct === "string" && direct.length > 0) {
		return direct;
	}
	throw new Error("invalid_exact_svm_payload_transaction");
}

function rejectNonSolana(
	paymentRequirements: PaymentRequirements | undefined
): string | null {
	const network = paymentRequirements?.network;
	if (!isSolanaNetwork(network)) {
		return `invalid_network: expected a Solana network (solana-devnet, solana, or solana:…), got ${String(network)}`;
	}
	return null;
}

export async function handleSupported(
	config: FacilitatorConfig
): Promise<SupportedResponse> {
	const kora = getKora(config);
	const { signer_address } = await kora.getPayerSigner();
	if (!signer_address) {
		throw new Error("Kora getPayerSigner returned an empty signer_address");
	}
	return {
		kinds: [
			{
				x402Version: 1,
				scheme: "exact",
				network: config.network || DEFAULT_NETWORK,
				extra: { feePayer: signer_address },
			},
		],
	};
}

export async function handleVerify(
	config: FacilitatorConfig,
	body: VerifyRequestBody
): Promise<VerifyResponse> {
	const networkError = rejectNonSolana(body.paymentRequirements);
	if (networkError) {
		return { isValid: false, invalidReason: networkError };
	}

	let transaction: string;
	try {
		transaction = extractSolanaTransaction(body.paymentPayload);
	} catch (error) {
		return {
			isValid: false,
			invalidReason:
				error instanceof Error
					? error.message
					: "invalid_exact_svm_payload_transaction",
		};
	}

	const kora = getKora(config);
	try {
		await kora.signTransaction({ transaction });
		return { isValid: true };
	} catch (error) {
		return {
			isValid: false,
			invalidReason:
				error instanceof Error ? error.message : "Kora validation failed",
		};
	}
}

export async function handleSettle(
	config: FacilitatorConfig,
	body: VerifyRequestBody
): Promise<SettleResponse> {
	const network = config.network || DEFAULT_NETWORK;
	const networkError = rejectNonSolana(body.paymentRequirements);
	if (networkError) {
		return {
			success: false,
			transaction: "",
			network,
			errorReason: networkError,
		};
	}

	let transaction: string;
	try {
		transaction = extractSolanaTransaction(body.paymentPayload);
	} catch (error) {
		return {
			success: false,
			transaction: "",
			network,
			errorReason:
				error instanceof Error
					? error.message
					: "invalid_exact_svm_payload_transaction",
		};
	}

	const kora = getKora(config);
	try {
		const result = await kora.signAndSendTransaction({ transaction });
		const signature = result.signature;
		if (!signature) {
			return {
				success: false,
				transaction: "",
				network,
				errorReason: "Kora signAndSendTransaction returned an empty signature",
			};
		}
		return {
			success: true,
			transaction: signature,
			network,
		};
	} catch (error) {
		return {
			success: false,
			transaction: "",
			network,
			errorReason:
				error instanceof Error ? error.message : "Kora validation failed",
		};
	}
}
