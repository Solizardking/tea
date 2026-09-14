import { createServer, type IncomingMessage, type Server } from "node:http";
import {
	handleSettle,
	handleSupported,
	handleVerify,
	type FacilitatorConfig,
	type VerifyRequestBody,
} from "./handlers.ts";

function readBody(req: IncomingMessage): Promise<string> {
	return new Promise((resolve, reject) => {
		const chunks: Buffer[] = [];
		req.on("data", (chunk) => {
			chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
		});
		req.on("end", () => resolve(Buffer.concat(chunks).toString("utf8")));
		req.on("error", reject);
	});
}

function json(
	res: { statusCode: number; setHeader: (k: string, v: string) => void; end: (b?: string) => void },
	status: number,
	body: unknown
) {
	const payload = JSON.stringify(body);
	res.statusCode = status;
	res.setHeader("content-type", "application/json");
	res.setHeader("access-control-allow-origin", "*");
	res.setHeader("access-control-allow-headers", "content-type, x-payment");
	res.setHeader("access-control-allow-methods", "GET, POST, OPTIONS");
	res.end(payload);
}

export function createFacilitatorServer(config: FacilitatorConfig): Server {
	return createServer(async (req, res) => {
		const url = new URL(req.url || "/", "http://localhost");
		const path = url.pathname.replace(/\/+$/, "") || "/";

		if (req.method === "OPTIONS") {
			res.statusCode = 204;
			res.setHeader("access-control-allow-origin", "*");
			res.setHeader("access-control-allow-headers", "content-type, x-payment");
			res.setHeader("access-control-allow-methods", "GET, POST, OPTIONS");
			res.end();
			return;
		}

		try {
			if (req.method === "GET" && (path === "/supported" || path === "/")) {
				if (path === "/") {
					json(res, 200, {
						service: "clawd-x402-facilitator",
						endpoints: ["/supported", "/verify", "/settle"],
					});
					return;
				}
				const supported = await handleSupported(config);
				json(res, 200, supported);
				return;
			}

			if (req.method === "POST" && path === "/verify") {
				const raw = await readBody(req);
				const body = (raw ? JSON.parse(raw) : {}) as VerifyRequestBody;
				const result = await handleVerify(config, body);
				json(res, 200, result);
				return;
			}

			if (req.method === "POST" && path === "/settle") {
				const raw = await readBody(req);
				const body = (raw ? JSON.parse(raw) : {}) as VerifyRequestBody;
				const result = await handleSettle(config, body);
				json(res, 200, result);
				return;
			}

			if (req.method === "GET" && path === "/health") {
				json(res, 200, { status: "ok" });
				return;
			}

			json(res, 404, { error: "not found" });
		} catch (error) {
			json(res, 500, {
				error: error instanceof Error ? error.message : String(error),
			});
		}
	});
}

export function listenFacilitator(
	config: FacilitatorConfig,
	port: number,
	host = "127.0.0.1"
): Promise<{ server: Server; url: string }> {
	const server = createFacilitatorServer(config);
	return new Promise((resolve, reject) => {
		server.once("error", reject);
		server.listen(port, host, () => {
			const addr = server.address();
			const bound =
				addr && typeof addr === "object" ? addr.port : port;
			resolve({ server, url: `http://${host}:${bound}` });
		});
	});
}
