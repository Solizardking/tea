#ifndef USER_TA_HEADER_DEFINES_H
#define USER_TA_HEADER_DEFINES_H

#include "clawd_ta.h"

#define TA_UUID CLAWD_TA_UUID

#define TA_FLAGS TA_FLAG_EXEC_DDR
#define TA_STACK_SIZE (8 * 1024)
#define TA_DATA_SIZE (32 * 1024)

#define TA_CURRENT_TA_EXT_PROPERTIES \
	{ "gp.ta.description", USER_TA_PROP_TYPE_STRING, \
		"CLAWD OP-TEE secp256k1 signer" }, \
	{ "gp.ta.version", USER_TA_PROP_TYPE_U32, &(const uint32_t){ 0x00000001 } }

#endif /* USER_TA_HEADER_DEFINES_H */
