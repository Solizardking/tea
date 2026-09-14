import assert from "node:assert/strict";
import { createServer, type Server } from "node:http";
import { describe, it } from "node:test";
import app from "../src";
import type { Env } from "../src/env";
import { CLAWD_MINT } from "../src/constants";

const SIGNER = "4utbbZp1KzxNDeZj2FMFyyUKg4jeTjNvgtxsRAGQMUWw";

function requestWithRawUrl(
	url: string,
	init?: RequestInit
): Request {
	const request = new Request(url, init);
	Object.defineProperty(request, "url", { value: url });
	return request;
}

function createEnv(overrides: Record<string, unknown> = {}) {
	let originFetchCalls = 0;
	const env = {
		ORIGIN_SERVICE: {
			fetch: async () => {
				originFetchCalls++;
				return new Response("origin content");
			},
		},
		JWT_SECRET: "test-jwt-secret-for-clawd-kora-proxy",
		NETWORK: "solana-devnet",
		PAY_TO: SIGNER,
		PAYMENT_ASSET: CLAWD_MINT,
		KORA_FEE_PAYER: SIGNER,
		PROTECTED_PATTERNS: [
			{
				pattern: "/premium/*",
				price: "$0.01",
				description: "Premium content",
			},
			{
				pattern: "/v1/chat/completions",
				price: "$0.0001",
				description: "CLAWD TEE inference",
			},
		],
		...overrides,
	} as unknown as Env;

	return { env, getOriginFetchCalls: () => originFetchCalls };
}

async function startMockFacilitator(): Promise<{
	url: string;
	server: Server;
	verifyCalls: number;
	settleCalls: number;
	lastSignature: string;
	close: () => Promise<void>;
}> {
	let verifyCalls = 0;
	let settleCalls = 0;
	let lastSignature = "";
	const server = createServer((req, res) => {
		const chunks: Buffer[] = [];
		req.on("data", (c) =>
			chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c))
		);
		req.on("end", () => {
			const path = (req.url || "").split("?")[0];
			res.setHeader("content-type", "application/json");
			if (req.method === "GET" && path === "/supported") {
				res.end(
					JSON.stringify({
						kinds: [
							{
								scheme: "exact",
								network: "solana-devnet",
								extra: { feePayer: SIGNER },
							},
						],
					})
				);
				return;
			}
			if (req.method === "POST" && path === "/verify") {
				verifyCalls++;
				res.end(JSON.stringify({ isValid: true }));
				return;
			}
			if (req.method === "POST" && path === "/settle") {
				settleCalls++;
				lastSignature = `sig_${verifyCalls}_${settleCalls}_${Date.now()}`;
				res.end(
					JSON.stringify({
						success: true,
						transaction: lastSignature,
						network: "solana-devnet",
					})
				);
				return;
			}
			res.statusCode = 404;
			res.end(JSON.stringify({ error: "not found" }));
		});
	});
	await new Promise<void>((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", () => resolve());
	});
	const addr = server.address();
	if (!addr || typeof addr === "string") {
		throw new Error("mock facilitator failed to bind");
	}
	return {
		url: `http://127.0.0.1:${addr.port}`,
		server,
		get verifyCalls() {
			return verifyCalls;
		},
		get settleCalls() {
			return settleCalls;
		},
		get lastSignature() {
			return lastSignature;
		},
		close: () =>
			new Promise((resolve) => {
				server.close(() => resolve());
			}),
	};
}

function encodePayment(transaction: string): string {
	return Buffer.from(
		JSON.stringify({
			x402Version: 1,
			scheme: "exact",
			network: "solana-devnet",
			payload: { transaction },
		}),
		"utf8"
	).toString("base64");
}

describe("x402 protected route matching", () => {
	it("rejects a leading dot segment instead of proxying protected content", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/./premium/secret"),
			env
		);

		assert.equal(response.status, 400);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("rejects parent segments instead of proxying protected content", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/free/../premium/secret"),
			env
		);

		assert.equal(response.status, 400);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("rejects percent-encoded dot segments", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/%2e/premium/secret"),
			env
		);

		assert.equal(response.status, 400);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("rejects repeated slashes instead of relying on origin normalization", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com//premium/secret"),
			env
		);

		assert.equal(response.status, 400);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("continues to protect canonical wildcard paths", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/premium/secret"),
			env
		);

		assert.equal(response.status, 402);
		const body = (await response.json()) as {
			accepts: Array<{ network: string; asset: string; scheme: string }>;
		};
		assert.equal(body.accepts[0].scheme, "exact");
		assert.equal(body.accepts[0].network, "solana-devnet");
		assert.equal(body.accepts[0].asset, CLAWD_MINT);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("protects the wildcard prefix itself", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/premium"),
			env
		);

		assert.equal(response.status, 402);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("matches percent-encoded ordinary characters using Hono semantics", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/%70remium/secret"),
			env
		);

		assert.equal(response.status, 402);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("does not protect paths that merely share a wildcard prefix", async () => {
		const { env, getOriginFetchCalls } = createEnv();

		const response = await app.fetch(
			requestWithRawUrl("https://example.com/premiumXYZ"),
			env
		);

		assert.equal(response.status, 200);
		assert.equal(await response.text(), "origin content");
		assert.equal(getOriginFetchCalls(), 1);
	});
});

describe("Clawd Solana x402 rail", () => {
	it("keeps /__x402/health unpaid", async () => {
		const { env } = createEnv();
		const response = await app.fetch(
			requestWithRawUrl("https://example.com/__x402/health"),
			env
		);
		assert.equal(response.status, 200);
		const body = (await response.json()) as { status: string };
		assert.equal(body.status, "ok");
	});

	it("returns Solana 402 on built-in /__x402/protected without payment", async () => {
		const { env, getOriginFetchCalls } = createEnv();
		const response = await app.fetch(
			requestWithRawUrl("https://example.com/__x402/protected"),
			env
		);
		assert.equal(response.status, 402);
		const body = (await response.json()) as {
			accepts: Array<{
				network: string;
				scheme: string;
				payTo: string;
				extra: { feePayer: string };
			}>;
		};
		assert.notEqual(body.accepts[0].network, "base-sepolia");
		assert.equal(body.accepts[0].network, "solana-devnet");
		assert.equal(body.accepts[0].scheme, "exact");
		assert.equal(body.accepts[0].payTo, SIGNER);
		assert.equal(body.accepts[0].extra.feePayer, SIGNER);
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("gates configured inference routes such as /v1/chat/completions", async () => {
		const { env, getOriginFetchCalls } = createEnv();
		const response = await app.fetch(
			requestWithRawUrl("https://example.com/v1/chat/completions", {
				method: "POST",
				headers: { "content-type": "application/json" },
				body: "{}",
			}),
			env
		);
		assert.equal(response.status, 402);
		const body = (await response.json()) as {
			accepts: Array<{ network: string }>;
		};
		assert.equal(body.accepts[0].network, "solana-devnet");
		assert.equal(getOriginFetchCalls(), 0);
	});

	it("does not payment-gate origin /health", async () => {
		const { env, getOriginFetchCalls } = createEnv();
		const response = await app.fetch(
			requestWithRawUrl("https://example.com/health"),
			env
		);
		assert.equal(response.status, 200);
		assert.equal(await response.text(), "origin content");
		assert.equal(getOriginFetchCalls(), 1);
	});

	it("grants access after Kora facilitator verify + settle", async () => {
		const facilitator = await startMockFacilitator();
		try {
			const { env, getOriginFetchCalls } = createEnv({
				FACILITATOR_URL: facilitator.url,
			});
			const payment = encodePayment("AQIDBAUG");
			const response = await app.fetch(
				requestWithRawUrl("https://example.com/__x402/protected", {
					headers: { "X-PAYMENT": payment },
				}),
				env
			);
			assert.equal(response.status, 200);
			const body = (await response.json()) as { message: string };
			assert.match(body.message, /Premium content accessed/i);
			assert.equal(facilitator.verifyCalls, 1);
			assert.equal(facilitator.settleCalls, 1);
			assert.ok(facilitator.lastSignature.length > 0);
			assert.ok(response.headers.get("X-PAYMENT-RESPONSE"));
			assert.equal(getOriginFetchCalls(), 0);
		} finally {
			await facilitator.close();
		}
	});
});
