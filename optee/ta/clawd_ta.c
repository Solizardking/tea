#include <stdint.h>
#include <string.h>

#include <tee_internal_api.h>
#include <tee_internal_api_extensions.h>

#include "clawd_ta.h"

#define CLAWD_SEED_OBJECT_ID "clawd-k1-seed-v1"
#define CLAWD_SEED_SIZE 32

/*
 * OP-TEE's standard crypto API availability varies by platform and crypto
 * provider. Keep the Solana-facing ABI stable here and wire the board-specific
 * secp256k1 implementation behind these two functions.
 */
static TEE_Result clawd_k1_public_key(const uint8_t seed[CLAWD_SEED_SIZE],
				      uint8_t pubkey[CLAWD_TA_PUBKEY_SIZE])
{
	(void)seed;
	TEE_MemFill(pubkey, 0, CLAWD_TA_PUBKEY_SIZE);
	return TEE_ERROR_NOT_SUPPORTED;
}

static TEE_Result clawd_k1_sign_recoverable(const uint8_t seed[CLAWD_SEED_SIZE],
					    const uint8_t payload_hash[CLAWD_TA_PAYLOAD_HASH_SIZE],
					    uint8_t signature[CLAWD_TA_SIGNATURE_SIZE])
{
	(void)seed;
	(void)payload_hash;
	TEE_MemFill(signature, 0, CLAWD_TA_SIGNATURE_SIZE);
	return TEE_ERROR_NOT_SUPPORTED;
}

static TEE_Result read_or_create_seed(uint8_t seed[CLAWD_SEED_SIZE])
{
	TEE_ObjectHandle object = TEE_HANDLE_NULL;
	uint32_t flags = TEE_DATA_FLAG_ACCESS_READ;
	uint32_t bytes_read = 0;
	TEE_Result result = TEE_SUCCESS;

	result = TEE_OpenPersistentObject(TEE_STORAGE_PRIVATE,
					 CLAWD_SEED_OBJECT_ID,
					 strlen(CLAWD_SEED_OBJECT_ID),
					 flags, &object);
	if (result == TEE_SUCCESS) {
		result = TEE_ReadObjectData(object, seed, CLAWD_SEED_SIZE, &bytes_read);
		TEE_CloseObject(object);
		if (result != TEE_SUCCESS)
			return result;
		if (bytes_read != CLAWD_SEED_SIZE)
			return TEE_ERROR_CORRUPT_OBJECT;
		return TEE_SUCCESS;
	}

	if (result != TEE_ERROR_ITEM_NOT_FOUND)
		return result;

	TEE_GenerateRandom(seed, CLAWD_SEED_SIZE);
	result = TEE_CreatePersistentObject(TEE_STORAGE_PRIVATE,
					   CLAWD_SEED_OBJECT_ID,
					   strlen(CLAWD_SEED_OBJECT_ID),
					   TEE_DATA_FLAG_ACCESS_READ |
					   TEE_DATA_FLAG_ACCESS_WRITE,
					   TEE_HANDLE_NULL,
					   seed,
					   CLAWD_SEED_SIZE,
					   &object);
	if (result == TEE_SUCCESS)
		TEE_CloseObject(object);

	return result;
}

static TEE_Result cmd_get_public_key(uint32_t param_types, TEE_Param params[4])
{
	const uint32_t expected = TEE_PARAM_TYPES(TEE_PARAM_TYPE_MEMREF_OUTPUT,
						 TEE_PARAM_TYPE_NONE,
						 TEE_PARAM_TYPE_NONE,
						 TEE_PARAM_TYPE_NONE);
	uint8_t seed[CLAWD_SEED_SIZE] = { 0 };
	TEE_Result result = TEE_SUCCESS;

	if (param_types != expected)
		return TEE_ERROR_BAD_PARAMETERS;
	if (params[0].memref.size < CLAWD_TA_PUBKEY_SIZE)
		return TEE_ERROR_SHORT_BUFFER;

	result = read_or_create_seed(seed);
	if (result != TEE_SUCCESS)
		return result;

	result = clawd_k1_public_key(seed, params[0].memref.buffer);
	if (result == TEE_SUCCESS)
		params[0].memref.size = CLAWD_TA_PUBKEY_SIZE;

	TEE_MemFill(seed, 0, sizeof(seed));
	return result;
}

static TEE_Result cmd_sign_payload(uint32_t param_types, TEE_Param params[4])
{
	const uint32_t expected = TEE_PARAM_TYPES(TEE_PARAM_TYPE_MEMREF_INPUT,
						 TEE_PARAM_TYPE_MEMREF_OUTPUT,
						 TEE_PARAM_TYPE_NONE,
						 TEE_PARAM_TYPE_NONE);
	uint8_t seed[CLAWD_SEED_SIZE] = { 0 };
	TEE_Result result = TEE_SUCCESS;

	if (param_types != expected)
		return TEE_ERROR_BAD_PARAMETERS;
	if (params[0].memref.size != CLAWD_TA_PAYLOAD_HASH_SIZE)
		return TEE_ERROR_BAD_PARAMETERS;
	if (params[1].memref.size < CLAWD_TA_SIGNATURE_SIZE)
		return TEE_ERROR_SHORT_BUFFER;

	result = read_or_create_seed(seed);
	if (result != TEE_SUCCESS)
		return result;

	result = clawd_k1_sign_recoverable(seed,
					  params[0].memref.buffer,
					  params[1].memref.buffer);
	if (result == TEE_SUCCESS)
		params[1].memref.size = CLAWD_TA_SIGNATURE_SIZE;

	TEE_MemFill(seed, 0, sizeof(seed));
	return result;
}

static TEE_Result cmd_get_attestation_binding(uint32_t param_types, TEE_Param params[4])
{
	const uint32_t expected = TEE_PARAM_TYPES(TEE_PARAM_TYPE_MEMREF_OUTPUT,
						 TEE_PARAM_TYPE_NONE,
						 TEE_PARAM_TYPE_NONE,
						 TEE_PARAM_TYPE_NONE);
	uint8_t seed[CLAWD_SEED_SIZE] = { 0 };
	TEE_OperationHandle digest = TEE_HANDLE_NULL;
	uint32_t digest_len = CLAWD_TA_ATTESTATION_BINDING_SIZE;
	TEE_Result result = TEE_SUCCESS;
	static const char domain[] = "CLAWD_OPTEE_BINDING_V1:";

	if (param_types != expected)
		return TEE_ERROR_BAD_PARAMETERS;
	if (params[0].memref.size < CLAWD_TA_ATTESTATION_BINDING_SIZE)
		return TEE_ERROR_SHORT_BUFFER;

	result = read_or_create_seed(seed);
	if (result != TEE_SUCCESS)
		return result;

	result = TEE_AllocateOperation(&digest, TEE_ALG_SHA256, TEE_MODE_DIGEST, 0);
	if (result != TEE_SUCCESS)
		goto out;

	TEE_DigestUpdate(digest, domain, sizeof(domain) - 1);
	result = TEE_DigestDoFinal(digest, seed, sizeof(seed),
				   params[0].memref.buffer, &digest_len);
	if (result == TEE_SUCCESS)
		params[0].memref.size = digest_len;

out:
	if (digest != TEE_HANDLE_NULL)
		TEE_FreeOperation(digest);
	TEE_MemFill(seed, 0, sizeof(seed));
	return result;
}

TEE_Result TA_CreateEntryPoint(void)
{
	return TEE_SUCCESS;
}

void TA_DestroyEntryPoint(void)
{
}

TEE_Result TA_OpenSessionEntryPoint(uint32_t param_types,
				    TEE_Param params[4],
				    void **session)
{
	(void)param_types;
	(void)params;
	*session = NULL;
	return TEE_SUCCESS;
}

void TA_CloseSessionEntryPoint(void *session)
{
	(void)session;
}

TEE_Result TA_InvokeCommandEntryPoint(void *session,
				      uint32_t command_id,
				      uint32_t param_types,
				      TEE_Param params[4])
{
	(void)session;

	switch (command_id) {
	case CLAWD_TA_CMD_GET_PUBLIC_KEY:
		return cmd_get_public_key(param_types, params);
	case CLAWD_TA_CMD_SIGN_PAYLOAD:
		return cmd_sign_payload(param_types, params);
	case CLAWD_TA_CMD_GET_ATTESTATION_BINDING:
		return cmd_get_attestation_binding(param_types, params);
	default:
		return TEE_ERROR_BAD_PARAMETERS;
	}
}
