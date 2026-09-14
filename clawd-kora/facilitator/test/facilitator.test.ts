import assert from "node:assert/strict";
import { after, describe, it } from "node:test";
import { createKoraClient } from "../src/kora.ts";
import {
	extractSolanaTransaction,
	handleSettle,
	handleSupported,
	handleVerify,
} from "../src/handlers.ts";
import { listenFacilitator } from "../src/server.ts";
import { startStubKora } from "./stub-kora.ts";

const SIGNER = "4utbbZp1KzxNDeZj2FMFyyUKg4jeTjNvgtxsRAGQMUWw";
const SAMPLE_TX = "AQIDBAUGBwgJCg==";

function solanaPayload(transaction = SAMPLE_TX) {
	return {
		x402Version: 1,
		scheme: "exact" as const,
		network: "solana-devnet",
		payload: { transaction },
	};
}

function solanaRequirements(network = "solana-devnet") {
	return {
		scheme: "exact" as const,
		network,
		maxAmountRequired: "10000",
		resource: "http://localhost/__x402/protected",
		description: "test",
		mimeType: "application/json",
		payTo: SIGNER,
		maxTimeoutSeconds: 60,
		asset: "8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump",
	};
}

describe("x402 facilitator handlers against stub Kora JSON-RPC", () => {
	it("extracts the Solana transaction from an x402 payment payload", () => {
		assert.equal(
			extractSolanaTransaction(solanaPayload("abc123")),
			"abc123"
		);
	});

	it("GET /supported advertises exact + Solana network + Kora feePayer", async () => {
		const stub = await startStubKora(SIGNER);
		try {
			const kora = createKoraClient({ rpcUrl: stub.url });
			const result = await handleSupported({
				koraRpcUrl: stub.url,
				network: "solana-devnet",
				kora,
			});
			assert.equal(result.kinds.length, 1);
			assert.equal(result.kinds[0].scheme, "exact");
			assert.equal(result.kinds[0].network, "solana-devnet");
			assert.equal(result.kinds[0].extra.feePayer, SIGNER);
			assert.ok(
				stub.calls.some((c) => c.method === "getPayerSigner"),
				"must call Kora getPayerSigner"
			);
		} finally {
			await stub.close();
		}
	});

	it("POST /verify calls Kora signTransaction and returns isValid: true", async () => {
		const stub = await startStubKora(SIGNER);
		try {
			const kora = createKoraClient({ rpcUrl: stub.url });
			const result = await handleVerify(
				{ koraRpcUrl: stub.url, network: "solana-devnet", kora },
				{
					paymentPayload: solanaPayload(SAMPLE_TX),
					paymentRequirements: solanaRequirements(),
				}
			);
			assert.equal(result.isValid, true);
			const signCall = stub.calls.find((c) => c.method === "signTransaction");
			assert.ok(signCall, "must call Kora signTransaction");
			assert.deepEqual(signCall.params, { transaction: SAMPLE_TX });
			assert.ok(
				!stub.calls.some((c) => c.method === "signAndSendTransaction"),
				"verify must not broadcast"
			);
		} finally {
			await stub.close();
		}
	});

	it("POST /settle calls Kora signAndSendTransaction and returns that signature", async () => {
		const stub = await startStubKora(SIGNER);
		try {
			const kora = createKoraClient({ rpcUrl: stub.url });
			const result = await handleSettle(
				{ koraRpcUrl: stub.url, network: "solana-devnet", kora },
				{
					paymentPayload: solanaPayload(SAMPLE_TX),
					paymentRequirements: solanaRequirements(),
				}
			);
			assert.equal(result.success, true);
			assert.ok(result.transaction.length > 0);
			assert.equal(result.transaction, stub.lastSignature);
			const sendCall = stub.calls.find(
				(c) => c.method === "signAndSendTransaction"
			);
			assert.ok(sendCall, "must call Kora signAndSendTransaction");
			assert.deepEqual(sendCall.params, { transaction: SAMPLE_TX });
		} finally {
			await stub.close();
		}
	});

	it("rejects a non-Solana paymentRequirements.network without calling Kora", async () => {
		const stub = await startStubKora(SIGNER);
		try {
			const kora = createKoraClient({ rpcUrl: stub.url });
			const verify = await handleVerify(
				{ koraRpcUrl: stub.url, network: "solana-devnet", kora },
				{
					paymentPayload: solanaPayload(),
					paymentRequirements: solanaRequirements("base-sepolia"),
				}
			);
			assert.equal(verify.isValid, false);
			assert.match(verify.invalidReason || "", /invalid_network/);

			const settle = await handleSettle(
				{ koraRpcUrl: stub.url, network: "solana-devnet", kora },
				{
					paymentPayload: solanaPayload(),
					paymentRequirements: solanaRequirements("eip155:84532"),
				}
			);
			assert.equal(settle.success, false);
			assert.match(settle.errorReason || "", /invalid_network/);
			assert.equal(stub.calls.length, 0);
		} finally {
			await stub.close();
		}
	});
});

describe("facilitator HTTP entry against stub Kora", () => {
	const closers: Array<() => Promise<void>> = [];
	after(async () => {
		for (const close of closers.reverse()) {
			await close();
		}
	});

	it("serves /supported /verify /settle over HTTP", async () => {
		const stub = await startStubKora(SIGNER);
		const { server, url } = await listenFacilitator(
			{ koraRpcUrl: stub.url, network: "solana-devnet" },
			0
		);
		closers.push(() => stub.close());
		closers.push(
			() =>
				new Promise((resolve) => {
					server.close(() => resolve());
				})
		);

		const supportedRes = await fetch(`${url}/supported`);
		assert.equal(supportedRes.status, 200);
		const supported = (await supportedRes.json()) as {
			kinds: Array<{
				scheme: string;
				network: string;
				extra: { feePayer: string };
			}>;
		};
		assert.equal(supported.kinds[0].scheme, "exact");
		assert.equal(supported.kinds[0].network, "solana-devnet");
		assert.equal(supported.kinds[0].extra.feePayer, SIGNER);

		const body = {
			x402Version: 1,
			paymentPayload: solanaPayload(),
			paymentRequirements: solanaRequirements(),
		};

		const verifyRes = await fetch(`${url}/verify`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(body),
		});
		assert.equal(verifyRes.status, 200);
		const verify = (await verifyRes.json()) as { isValid: boolean };
		assert.equal(verify.isValid, true);

		const settleRes = await fetch(`${url}/settle`, {
			method: "POST",
			headers: { "content-type": "application/json" },
			body: JSON.stringify(body),
		});
		assert.equal(settleRes.status, 200);
		const settle = (await settleRes.json()) as {
			success: boolean;
			transaction: string;
		};
		assert.equal(settle.success, true);
		assert.ok(settle.transaction.length > 0);
		assert.equal(settle.transaction, stub.lastSignature);
	});
});
