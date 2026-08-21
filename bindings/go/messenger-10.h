#ifndef TYPIKON_LAYER_BRIDGE_H
#define TYPIKON_LAYER_BRIDGE_H
#include <stddef.h>
#include <stdint.h>

typedef struct { int32_t status; uint8_t *data_ptr; size_t data_len; size_t data_capacity; uint8_t *error_ptr; size_t error_len; size_t error_capacity; } TypikonBridgeResult;

void typikon_free_bytes(uint8_t *ptr, size_t len, size_t capacity);

int32_t typikon_10_user_flags_validate_borrowed(const uint8_t *input, size_t len);
int32_t typikon_10_presence_validate_borrowed(const uint8_t *input, size_t len);
int32_t typikon_10_user_validate_borrowed(const uint8_t *input, size_t len);
int32_t typikon_10_attachment_validate_borrowed(const uint8_t *input, size_t len);
int32_t typikon_10_message_validate_borrowed(const uint8_t *input, size_t len);
int32_t typikon_10_update_validate_borrowed(const uint8_t *input, size_t len);

#endif
