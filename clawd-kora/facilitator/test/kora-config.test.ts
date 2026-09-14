import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, it } from "node:test";
import { CLAWD_MINT } from "../src/constants.ts";

const here = dirname(fileURLToPath(import.meta.url));
const toml = readFileSync(resolve(here, "../../kora.toml"), "utf8");

describe("Clawd Kora operator config", () => {
	it("allowlists $CLAWD as an SPL fee-payment token", () => {
		assert.match(toml, /allowed_spl_paid_tokens\s*=\s*\[[^\]]*8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump/s);
		assert.equal(CLAWD_MINT, "8cHzQHUS2s2h8TzCmfqPKYiM4dSt4roa3n7MyRLApump");
	});

	it("allowlists System, Token, Associated Token, and Compute Budget programs", () => {
		assert.match(toml, /11111111111111111111111111111111/);
		assert.match(toml, /TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA/);
		assert.match(toml, /ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL/);
		assert.match(toml, /ComputeBudget111111111111111111111111111111/);
	});
});
