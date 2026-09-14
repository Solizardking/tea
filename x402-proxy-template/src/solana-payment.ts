/**
 * Solana/Kora x402 payment middleware.
 *
 * x402-hono 1.0.0 can advertise SVM networks, but unpaid SVM 402s call the
 * facilitator `/supported` (500 if it is down) and decode X-PAYMENT as EVM.
 * This adapter returns Solana `accepts` locally and verify/settles via Kora.
 */

import type { Context, Next } from "hono";
import { CLAWD_MINT, isSolanaNetwork } from "./constants";
import type { AppContext } from "./env";
import type { ProtectedRouteConfig } from "./auth";

const X402_VERSION = 1;

export interface SolanaPaymentAccepts {
	scheme: "exact";
	network: string;
	maxAmountRequired: string;
	resource: string;
	description: string;
	mimeType: string;
	payTo: string;
	maxTimeoutSeconds: number;
	asset: string;
	extra: { feePayer: string };
}

function usdToAtomic(price: string, decimals = 6): string {
	const numeric = Number(String(price).replace(/\$/g, "").trim());
	if (!Number.isFinite(numeric) || numeric < 0) {
		throw new Error(`Invalid price: ${price}`);
	}
	return Math.round(numeric * 10 ** decimals).toString();
}

function decodeXPayment(header: string): {
	x402Version?: number;
	scheme?: string;
	network?: string;
	payload?: { transaction?: string };
} {
	const tryParse = (raw: string) => JSON.parse(raw) as ReturnType<typeof decodeXPayment>;
	try {
		return tryParse(atob(header));
	} catch {
		return tryParse(header);
	}
}

function encodeSettleResponse(body: unknown): string {
	const json = JSON.stringify(body);
	if (typeof btoa === "function") {
		return btoa(json);
	}
	return Buffer.from(json, "utf8").toString("base64");
}

async function fetchFeePayer(
	facilitatorUrl: string | undefined,
	fallback: string
): Promise<string> {
	if (!facilitatorUrl) return fallback;
	try {
		const res = await fetch(`${facilitatorUrl.replace(/\/+$/, "")}/supported`);
		if (!res.ok) return fallback;
		const body = (await res.json()) as {
			kinds?: Array<{ extra?: { feePayer?: string } }>;
		};
		const feePayer = body.kinds?.[0]?.extra?.feePayer;
		return feePayer && feePayer.length > 0 ? feePayer : fallback;
	} catch {
		return fallback;
	}
}

export async function buildSolanaAccepts(
	c: Context<AppContext>,
	config: ProtectedRouteConfig
): Promise<SolanaPaymentAccepts> {
	const network = c.env.NETWORK;
	if (!isSolanaNetwork(network)) {
		throw new Error(
			`Unsupported network for Clawd Kora rail: ${network}. Use solana-devnet or solana.`
		);
	}
	const payTo = c.env.PAY_TO;
	const asset = c.env.PAYMENT_ASSET || CLAWD_MINT;
	const feePayer = await fetchFeePayer(
		c.env.FACILITATOR_URL,
		c.env.KORA_FEE_PAYER || payTo
	);
	return {
		scheme: "exact",
		network,
		maxAmountRequired: usdToAtomic(config.price),
		resource: c.req.url,
		description: config.description,
		mimeType: "application/json",
		payTo,
		maxTimeoutSeconds: 60,
		asset,
		extra: { feePayer },
	};
}

/**
 * Cookie-or-pay middleware for Solana x402 (Kora facilitator).
 */
export function solanaPaymentMiddleware(config: ProtectedRouteConfig) {
	return async (c: Context<AppContext>, next: Next) => {
		const accepts = await buildSolanaAccepts(c, config);
		const paymentHeader = c.req.header("X-PAYMENT");

		if (!paymentHeader) {
			return c.json(
				{
					error: "X-PAYMENT header is required",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				402
			);
		}

		let decoded: ReturnType<typeof decodeXPayment>;
		try {
			decoded = decodeXPayment(paymentHeader);
		} catch (error) {
			return c.json(
				{
					error:
						error instanceof Error
							? error.message
							: "Invalid or malformed payment header",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				402
			);
		}

		const transaction = decoded.payload?.transaction;
		if (!transaction) {
			return c.json(
				{
					error: "invalid_exact_svm_payload_transaction",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				402
			);
		}

		const facilitatorUrl = (c.env.FACILITATOR_URL || "").replace(/\/+$/, "");
		if (!facilitatorUrl) {
			return c.json(
				{
					error: "Server misconfigured: FACILITATOR_URL not set",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				500
			);
		}

		const verifyBody = {
			x402Version: decoded.x402Version ?? X402_VERSION,
			paymentPayload: decoded,
			paymentRequirements: accepts,
		};

		try {
			const verifyRes = await fetch(`${facilitatorUrl}/verify`, {
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify(verifyBody),
			});
			const verification = (await verifyRes.json()) as {
				isValid?: boolean;
				invalidReason?: string;
			};
			if (!verification.isValid) {
				return c.json(
					{
						error: verification.invalidReason || "Payment verification failed",
						accepts: [accepts],
						x402Version: X402_VERSION,
					},
					402
				);
			}
		} catch (error) {
			return c.json(
				{
					error:
						error instanceof Error
							? error.message
							: "Payment verification failed",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				402
			);
		}

		// next() here is the proxy's JWT-issuance hook, not the origin handler.
		await next();

		try {
			const settleRes = await fetch(`${facilitatorUrl}/settle`, {
				method: "POST",
				headers: { "content-type": "application/json" },
				body: JSON.stringify(verifyBody),
			});
			const settlement = (await settleRes.json()) as {
				success?: boolean;
				transaction?: string;
				errorReason?: string;
				network?: string;
			};
			if (!settlement.success) {
				throw new Error(settlement.errorReason || "Failed to settle payment");
			}
			c.header("X-PAYMENT-RESPONSE", encodeSettleResponse(settlement));
		} catch (error) {
			return c.json(
				{
					error:
						error instanceof Error
							? error.message
							: "Failed to settle payment",
					accepts: [accepts],
					x402Version: X402_VERSION,
				},
				402
			);
		}
	};
}
