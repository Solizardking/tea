#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <tee_client_api.h>

#include "clawd_ta.h"

static int hex_nibble(char c)
{
	if (c >= '0' && c <= '9')
		return c - '0';
	if (c >= 'a' && c <= 'f')
		return c - 'a' + 10;
	if (c >= 'A' && c <= 'F')
		return c - 'A' + 10;
	return -1;
}

static bool parse_hex(const char *hex, uint8_t *out, size_t out_len)
{
	size_t i = 0;

	if (strlen(hex) != out_len * 2)
		return false;

	for (i = 0; i < out_len; i++) {
		int hi = hex_nibble(hex[i * 2]);
		int lo = hex_nibble(hex[i * 2 + 1]);

		if (hi < 0 || lo < 0)
			return false;
		out[i] = (uint8_t)((hi << 4) | lo);
	}

	return true;
}

static void print_hex(const uint8_t *buf, size_t len)
{
	size_t i = 0;

	for (i = 0; i < len; i++)
		printf("%02x", buf[i]);
}

static void usage(const char *argv0)
{
	fprintf(stderr, "usage: %s --payload-hash <32-byte-hex>\n", argv0);
}

static TEEC_Result invoke_get_public_key(TEEC_Session *session, uint8_t pubkey[CLAWD_TA_PUBKEY_SIZE])
{
	TEEC_Operation op = { 0 };
	uint32_t origin = 0;

	op.paramTypes = TEEC_PARAM_TYPES(TEEC_MEMREF_TEMP_OUTPUT, TEEC_NONE,
					 TEEC_NONE, TEEC_NONE);
	op.params[0].tmpref.buffer = pubkey;
	op.params[0].tmpref.size = CLAWD_TA_PUBKEY_SIZE;

	return TEEC_InvokeCommand(session, CLAWD_TA_CMD_GET_PUBLIC_KEY, &op, &origin);
}

static TEEC_Result invoke_sign_payload(TEEC_Session *session,
				       const uint8_t payload_hash[CLAWD_TA_PAYLOAD_HASH_SIZE],
				       uint8_t signature[CLAWD_TA_SIGNATURE_SIZE])
{
	TEEC_Operation op = { 0 };
	uint32_t origin = 0;

	op.paramTypes = TEEC_PARAM_TYPES(TEEC_MEMREF_TEMP_INPUT,
					 TEEC_MEMREF_TEMP_OUTPUT,
					 TEEC_NONE, TEEC_NONE);
	op.params[0].tmpref.buffer = (void *)payload_hash;
	op.params[0].tmpref.size = CLAWD_TA_PAYLOAD_HASH_SIZE;
	op.params[1].tmpref.buffer = signature;
	op.params[1].tmpref.size = CLAWD_TA_SIGNATURE_SIZE;

	return TEEC_InvokeCommand(session, CLAWD_TA_CMD_SIGN_PAYLOAD, &op, &origin);
}

int main(int argc, char **argv)
{
	TEEC_UUID uuid = CLAWD_TA_UUID;
	TEEC_Context context = { 0 };
	TEEC_Session session = { 0 };
	TEEC_Result result = TEEC_SUCCESS;
	uint32_t origin = 0;
	uint8_t payload_hash[CLAWD_TA_PAYLOAD_HASH_SIZE] = { 0 };
	uint8_t pubkey[CLAWD_TA_PUBKEY_SIZE] = { 0 };
	uint8_t signature[CLAWD_TA_SIGNATURE_SIZE] = { 0 };

	if (argc != 3 || strcmp(argv[1], "--payload-hash") != 0) {
		usage(argv[0]);
		return EXIT_FAILURE;
	}

	if (!parse_hex(argv[2], payload_hash, sizeof(payload_hash))) {
		fprintf(stderr, "payload hash must be exactly 64 hex characters\n");
		return EXIT_FAILURE;
	}

	result = TEEC_InitializeContext(NULL, &context);
	if (result != TEEC_SUCCESS) {
		fprintf(stderr, "TEEC_InitializeContext failed: 0x%x\n", result);
		return EXIT_FAILURE;
	}

	result = TEEC_OpenSession(&context, &session, &uuid, TEEC_LOGIN_PUBLIC,
				  NULL, NULL, &origin);
	if (result != TEEC_SUCCESS) {
		fprintf(stderr, "TEEC_OpenSession failed: 0x%x origin=%u\n",
			result, origin);
		TEEC_FinalizeContext(&context);
		return EXIT_FAILURE;
	}

	result = invoke_get_public_key(&session, pubkey);
	if (result != TEEC_SUCCESS) {
		fprintf(stderr, "GET_PUBLIC_KEY failed: 0x%x\n", result);
		TEEC_CloseSession(&session);
		TEEC_FinalizeContext(&context);
		return EXIT_FAILURE;
	}

	result = invoke_sign_payload(&session, payload_hash, signature);
	if (result != TEEC_SUCCESS) {
		fprintf(stderr, "SIGN_PAYLOAD failed: 0x%x\n", result);
		TEEC_CloseSession(&session);
		TEEC_FinalizeContext(&context);
		return EXIT_FAILURE;
	}

	printf("{\"provider\":\"optee\",\"payload_hash\":\"");
	print_hex(payload_hash, sizeof(payload_hash));
	printf("\",\"public_key\":\"");
	print_hex(pubkey, sizeof(pubkey));
	printf("\",\"signature\":\"");
	print_hex(signature, sizeof(signature));
	printf("\"}\n");

	TEEC_CloseSession(&session);
	TEEC_FinalizeContext(&context);
	return EXIT_SUCCESS;
}
