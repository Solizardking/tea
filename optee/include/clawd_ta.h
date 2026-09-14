#ifndef CLAWD_TA_H
#define CLAWD_TA_H

#include <stddef.h>
#include <stdint.h>

/*
 * CLAWD OP-TEE signer trusted application.
 *
 * UUID: 4b534c41-5744-5445-452d-5349474e4552
 * The host app opens this TA and requests a secp256k1 identity plus a
 * recoverable signature over the Solana StoreProofV2 signed_payload_hash.
 */
#define CLAWD_TA_UUID \
	{ 0x4b534c41, 0x5744, 0x5445, \
		{ 0x45, 0x2d, 0x53, 0x49, 0x47, 0x4e, 0x45, 0x52 } }

#define CLAWD_TA_CMD_GET_PUBLIC_KEY 0
#define CLAWD_TA_CMD_SIGN_PAYLOAD 1
#define CLAWD_TA_CMD_GET_ATTESTATION_BINDING 2

#define CLAWD_TA_PAYLOAD_HASH_SIZE 32
#define CLAWD_TA_PUBKEY_SIZE 65
#define CLAWD_TA_SIGNATURE_SIZE 65
#define CLAWD_TA_ATTESTATION_BINDING_SIZE 32

#endif /* CLAWD_TA_H */
