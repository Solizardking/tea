import { createServer, type Server } from "node:http";
import { randomBytes } from "node:crypto";

export interface StubKoraCall {
	method: string;
	params: unknown;
}

export interface StubKora {
	url: string;
	server: Server;
	calls: StubKoraCall[];
	signerAddress: string;
	lastSignature: string | null;
	close: () => Promise<void>;
}

function base58ish(bytes: Buffer): string {
	const alphabet =
		"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
	let out = "";
	for (const b of bytes) {
		out += alphabet[b % alphabet.length];
	}
	return out;
}

export function startStubKora(signerAddress: string): Promise<StubKora> {
	const calls: StubKoraCall[] = [];
	let lastSignature: string | null = null;

	const server = createServer((req, res) => {
		const chunks: Buffer[] = [];
		req.on("data", (c) =>
			chunks.push(Buffer.isBuffer(c) ? c : Buffer.from(c))
		);
		req.on("end", () => {
			let id: unknown = 1;
			try {
				const body = JSON.parse(Buffer.concat(chunks).toString("utf8")) as {
					id?: unknown;
					method?: string;
					params?: unknown;
				};
				id = body.id ?? 1;
				const method = body.method || "";
				const params = body.params ?? {};
				calls.push({ method, params });

				let result: unknown;
				if (method === "getPayerSigner") {
					result = { signer_address: signerAddress };
				} else if (method === "getConfig") {
					result = {
						validation_config: {
							allowed_spl_paid_tokens: [
								"8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump",
							],
						},
					};
				} else if (method === "signTransaction") {
					const tx =
						params &&
						typeof params === "object" &&
						"transaction" in params
							? String(
									(params as { transaction?: string }).transaction || ""
								)
							: "";
					result = {
						signed_transaction: tx || "signed",
						signer_pubkey: signerAddress,
						signature: base58ish(randomBytes(32)),
					};
				} else if (method === "signAndSendTransaction") {
					lastSignature = base58ish(randomBytes(32));
					result = {
						signature: lastSignature,
						signed_transaction: "sent",
					};
				} else {
					res.statusCode = 200;
					res.setHeader("content-type", "application/json");
					res.end(
						JSON.stringify({
							jsonrpc: "2.0",
							id,
							error: { code: -32601, message: `Unknown method ${method}` },
						})
					);
					return;
				}

				res.statusCode = 200;
				res.setHeader("content-type", "application/json");
				res.end(JSON.stringify({ jsonrpc: "2.0", id, result }));
			} catch (error) {
				res.statusCode = 200;
				res.setHeader("content-type", "application/json");
				res.end(
					JSON.stringify({
						jsonrpc: "2.0",
						id,
						error: {
							code: -32700,
							message: error instanceof Error ? error.message : "parse error",
						},
					})
				);
			}
		});
	});

	return new Promise((resolve, reject) => {
		server.once("error", reject);
		server.listen(0, "127.0.0.1", () => {
			const addr = server.address();
			if (!addr || typeof addr === "string") {
				reject(new Error("stub kora failed to bind"));
				return;
			}
			const stub: StubKora = {
				url: `http://127.0.0.1:${addr.port}`,
				server,
				calls,
				signerAddress,
				get lastSignature() {
					return lastSignature;
				},
				close: () =>
					new Promise((resClose) => {
						server.close(() => resClose());
					}),
			};
			resolve(stub);
		});
	});
}
