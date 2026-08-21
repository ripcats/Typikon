package typikon

/*
#cgo CFLAGS: -I.
#include "messenger-10.h"
*/
import "C"

import (
    "encoding/json"
    "fmt"
    "unsafe"
)

func bridgeResult(result C.TypikonBridgeResult) ([]byte, error) {
    defer C.typikon_free_bytes(result.data_ptr, result.data_len, result.data_capacity)
    defer C.typikon_free_bytes(result.error_ptr, result.error_len, result.error_capacity)
    if result.status != 0 { return nil, fmt.Errorf("native bridge error: %s", string(C.GoBytes(unsafe.Pointer(result.error_ptr), C.int(result.error_len)))) }
    return C.GoBytes(unsafe.Pointer(result.data_ptr), C.int(result.data_len)), nil
}
func bridgePtr(data []byte) *C.uint8_t { if len(data) == 0 { return nil }; return (*C.uint8_t)(unsafe.Pointer(&data[0])) }

type UserFlags uint16

func EncodeUserFlags(value UserFlags) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_user_flags_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodeUserFlags(wire []byte) (UserFlags, error) { var value UserFlags; result := C.typikon_10_user_flags_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidateUserFlags(wire []byte) error { if C.typikon_10_user_flags_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid UserFlags wire") }; return nil }

type Presence string

func EncodePresence(value Presence) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_presence_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodePresence(wire []byte) (Presence, error) { var value Presence; result := C.typikon_10_presence_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidatePresence(wire []byte) error { if C.typikon_10_presence_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid Presence wire") }; return nil }

type User struct {
    Id uint64 `json:"id"`
    Username string `json:"username"`
    DisplayName string `json:"display_name"`
    Flags UserFlags `json:"flags"`
    AvatarUrl *string `json:"avatar_url"`
    Presence Presence `json:"presence"`
    Roles []string `json:"roles"`
}

func EncodeUser(value User) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_user_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodeUser(wire []byte) (User, error) { var value User; result := C.typikon_10_user_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidateUser(wire []byte) error { if C.typikon_10_user_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid User wire") }; return nil }

type Attachment struct {
    Id uint64 `json:"id"`
    Name string `json:"name"`
    MimeType string `json:"mime_type"`
    Size uint64 `json:"size"`
}

func EncodeAttachment(value Attachment) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_attachment_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodeAttachment(wire []byte) (Attachment, error) { var value Attachment; result := C.typikon_10_attachment_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidateAttachment(wire []byte) error { if C.typikon_10_attachment_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid Attachment wire") }; return nil }

type Message struct {
    Id uint64 `json:"id"`
    ChatId uint64 `json:"chat_id"`
    Sender User `json:"sender"`
    Text string `json:"text"`
    Attachments []Attachment `json:"attachments"`
    Metadata map[string]string `json:"metadata"`
}

func EncodeMessage(value Message) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_message_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodeMessage(wire []byte) (Message, error) { var value Message; result := C.typikon_10_message_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidateMessage(wire []byte) error { if C.typikon_10_message_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid Message wire") }; return nil }

type Update map[string]json.RawMessage

func EncodeUpdate(value Update) ([]byte, error) { input, err := json.Marshal(value); if err != nil { return nil, err }; result := C.typikon_10_update_encode_json(bridgePtr(input), C.size_t(len(input))); return bridgeResult(result) }
func DecodeUpdate(wire []byte) (Update, error) { var value Update; result := C.typikon_10_update_decode_json(bridgePtr(wire), C.size_t(len(wire))); data, err := bridgeResult(result); if err != nil { return value, err }; err = json.Unmarshal(data, &value); return value, err }

func ValidateUpdate(wire []byte) error { if C.typikon_10_update_validate_borrowed(bridgePtr(wire), C.size_t(len(wire))) != 0 { return fmt.Errorf("invalid Update wire") }; return nil }

