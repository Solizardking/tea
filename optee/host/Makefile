CC ?= $(CROSS_COMPILE)gcc
CFLAGS ?= -O2 -Wall -Wextra -Werror

TEEC_EXPORT ?= /usr
OUT ?= clawd-optee-signer

CFLAGS += -I../include -I$(TEEC_EXPORT)/include
LDLIBS += -L$(TEEC_EXPORT)/lib -lteec

.PHONY: all clean

all: $(OUT)

$(OUT): main.c ../include/clawd_ta.h
	$(CC) $(CFLAGS) -o $@ main.c $(LDLIBS)

clean:
	rm -f $(OUT)
