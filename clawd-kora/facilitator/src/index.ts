import { loadEnv } from "./load-env.ts";
import {
	DEFAULT_FACILITATOR_PORT,
	DEFAULT_KORA_RPC_URL,
	DEFAULT_NETWORK,
} from "./constants.ts";
import { listenFacilitator } from "./server.ts";

loadEnv();

const port = Number(process.env.FACILITATOR_PORT || DEFAULT_FACILITATOR_PORT);
const koraRpcUrl = process.env.KORA_RPC_URL || DEFAULT_KORA_RPC_URL;
const network = process.env.NETWORK || DEFAULT_NETWORK;
const koraApiKey = process.env.KORA_API_KEY;

const { url } = await listenFacilitator(
	{ koraRpcUrl, koraApiKey, network },
	port
);
console.log(`Clawd x402 facilitator listening at ${url}`);
console.log(`  Kora RPC: ${koraRpcUrl}`);
console.log(`  Network:  ${network}`);
