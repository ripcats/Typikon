#ifndef TYPIKON_H
#define TYPIKON_H

#include <stddef.h>
#include <stdint.h>

#define TYPIKON_ABI_VERSION 1u
#define TYPIKON_LAYER_UNSUPPORTED (-1)
#define TYPIKON_INVALID_ARGUMENT (-2)

uint16_t typikon_abi_version(void);
int32_t typikon_negotiate_layer(uint16_t requested,
                                const uint16_t *supported,
                                size_t count);
void typikon_free_bytes(uint8_t *ptr, size_t len, size_t capacity);

#endif
