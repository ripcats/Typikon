package typikon

/*
#cgo CFLAGS: -I.
#include "messenger-10.h"
*/
import "C"

import (
	"encoding/binary"
	"fmt"
	"math"
	"sort"
	"unsafe"
)

const maxPacketSize = 4 << 20
const maxItems = 1_000_000

type wireEncoder struct{ b []byte }

func (e *wireEncoder) raw(v []byte) { e.b = append(e.b, v...) }
func (e *wireEncoder) u8(v uint8)   { e.b = append(e.b, v) }
func (e *wireEncoder) u16(v uint16) {
	var x [2]byte
	binary.LittleEndian.PutUint16(x[:], v)
	e.raw(x[:])
}
func (e *wireEncoder) u32(v uint32) {
	var x [4]byte
	binary.LittleEndian.PutUint32(x[:], v)
	e.raw(x[:])
}
func (e *wireEncoder) u64(v uint64) {
	var x [8]byte
	binary.LittleEndian.PutUint64(x[:], v)
	e.raw(x[:])
}
func (e *wireEncoder) i8(v int8)   { e.u8(uint8(v)) }
func (e *wireEncoder) i16(v int16) { e.u16(uint16(v)) }
func (e *wireEncoder) i32(v int32) { e.u32(uint32(v)) }
func (e *wireEncoder) i64(v int64) { e.u64(uint64(v)) }
func (e *wireEncoder) bool(v bool) {
	if v {
		e.u8(1)
	} else {
		e.u8(0)
	}
}
func (e *wireEncoder) f32(v float32) { e.u32(math.Float32bits(v)) }
func (e *wireEncoder) f64(v float64) { e.u64(math.Float64bits(v)) }
func (e *wireEncoder) varint(v uint64) {
	for v >= 0x80 {
		e.u8(byte(v) | 0x80)
		v >>= 7
	}
	e.u8(byte(v))
}
func (e *wireEncoder) bytes(v []byte)  { e.varint(uint64(len(v))); e.raw(v) }
func (e *wireEncoder) string(v string) { e.bytes([]byte(v)) }
func (e *wireEncoder) finish() ([]byte, error) {
	if len(e.b) > maxPacketSize {
		return nil, fmt.Errorf("packet exceeds limit")
	}
	return e.b, nil
}

type wireDecoder struct {
	b []byte
	p int
}

func (d *wireDecoder) take(n int) ([]byte, error) {
	if n < 0 || d.p > len(d.b)-n {
		return nil, fmt.Errorf("truncated wire")
	}
	v := d.b[d.p : d.p+n]
	d.p += n
	return v, nil
}
func (d *wireDecoder) u8() (uint8, error) {
	v, e := d.take(1)
	if e != nil {
		return 0, e
	}
	return v[0], nil
}
func (d *wireDecoder) u16() (uint16, error) {
	v, e := d.take(2)
	if e != nil {
		return 0, e
	}
	return binary.LittleEndian.Uint16(v), nil
}
func (d *wireDecoder) u32() (uint32, error) {
	v, e := d.take(4)
	if e != nil {
		return 0, e
	}
	return binary.LittleEndian.Uint32(v), nil
}
func (d *wireDecoder) u64() (uint64, error) {
	v, e := d.take(8)
	if e != nil {
		return 0, e
	}
	return binary.LittleEndian.Uint64(v), nil
}
func (d *wireDecoder) i8() (int8, error)     { v, e := d.u8(); return int8(v), e }
func (d *wireDecoder) i16() (int16, error)   { v, e := d.u16(); return int16(v), e }
func (d *wireDecoder) i32() (int32, error)   { v, e := d.u32(); return int32(v), e }
func (d *wireDecoder) i64() (int64, error)   { v, e := d.u64(); return int64(v), e }
func (d *wireDecoder) bool() (bool, error)   { v, e := d.u8(); return v != 0, e }
func (d *wireDecoder) f32() (float32, error) { v, e := d.u32(); return math.Float32frombits(v), e }
func (d *wireDecoder) f64() (float64, error) { v, e := d.u64(); return math.Float64frombits(v), e }
func (d *wireDecoder) varint() (uint64, error) {
	var v uint64
	for i := 0; i < 10; i++ {
		b, e := d.u8()
		if e != nil {
			return 0, e
		}
		if i == 9 && b > 1 {
			return 0, fmt.Errorf("invalid varint")
		}
		v |= uint64(b&0x7f) << (7 * i)
		if b < 0x80 {
			return v, nil
		}
	}
	return 0, fmt.Errorf("varint overflow")
}
func (d *wireDecoder) bytes() ([]byte, error) {
	n, e := d.varint()
	if e != nil || n > maxPacketSize || n > uint64(len(d.b)-d.p) {
		return nil, fmt.Errorf("invalid byte field")
	}
	return d.take(int(n))
}
func (d *wireDecoder) string() (string, error) { v, e := d.bytes(); return string(v), e }
func (d *wireDecoder) done() error {
	if d.p != len(d.b) {
		return fmt.Errorf("trailing bytes")
	}
	return nil
}
func count(n uint64) (int, error) {
	if n > maxItems || n > uint64(^uint(0)>>1) {
		return 0, fmt.Errorf("collection too large")
	}
	return int(n), nil
}
func cid(d *wireDecoder, want []byte) error {
	got, e := d.take(8)
	if e != nil || string(got) != string(want) {
		return fmt.Errorf("invalid constructor ID")
	}
	return nil
}
func bridgePtr(data []byte) *C.uint8_t {
	if len(data) == 0 {
		return nil
	}
	return (*C.uint8_t)(unsafe.Pointer(&data[0]))
}

type UserFlags uint16

func encode_user_flags(e *wireEncoder, v UserFlags)       { e.u16(uint16(v)) }
func decode_user_flags(d *wireDecoder) (UserFlags, error) { v, e := d.u16(); return UserFlags(v), e }
func EncodeUserFlags(v UserFlags) ([]byte, error) {
	e := wireEncoder{}
	encode_user_flags(&e, v)
	return e.finish()
}
func DecodeUserFlags(b []byte) (UserFlags, error) {
	d := wireDecoder{b: b}
	v, e := decode_user_flags(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateUserFlags(b []byte) error {
	if C.typikon_10_user_flags_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid UserFlags wire")
	}
	return nil
}

type Presence string

func encode_presence(e *wireEncoder, v Presence) {
	switch v {
	case "Online":
		e.u64(0)
	case "Away":
		e.u64(1)
	case "Offline":
		e.u64(2)
	}
}
func decode_presence(d *wireDecoder) (Presence, error) {
	v, e := d.u64()
	if e != nil {
		return "", e
	}
	switch v {
	case 0:
		return "Online", nil
	case 1:
		return "Away", nil
	case 2:
		return "Offline", nil
	}
	return "", fmt.Errorf("invalid Presence")
}
func EncodePresence(v Presence) ([]byte, error) {
	e := wireEncoder{}
	encode_presence(&e, v)
	return e.finish()
}
func DecodePresence(b []byte) (Presence, error) {
	d := wireDecoder{b: b}
	v, e := decode_presence(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidatePresence(b []byte) error {
	if C.typikon_10_presence_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid Presence wire")
	}
	return nil
}

type User struct {
	Id          uint64
	Username    string
	DisplayName string
	Flags       UserFlags
	AvatarUrl   *string
	Presence    Presence
	Roles       []string
}

var UserCID = []byte{0xac, 0xb3, 0x8d, 0xa6, 0x7a, 0x71, 0x20, 0x58}

func encode_user(e *wireEncoder, v User) {
	e.raw(UserCID)
	e.u64(v.Id)
	e.string(v.Username)
	e.string(v.DisplayName)
	encode_user_flags(e, v.Flags)
	if v.Flags&(1<<2) != 0 {
		e.string(*v.AvatarUrl)
	}
	encode_presence(e, v.Presence)
	e.varint(uint64(len(v.Roles)))
	for _, item := range v.Roles {
		e.string(item)
	}
}
func decode_user(d *wireDecoder) (User, error) {
	var v User
	if e := cid(d, UserCID); e != nil {
		return v, e
	}
	var e error
	v.Id, e = d.u64()
	if e != nil {
		return v, e
	}
	v.Username, e = d.string()
	if e != nil {
		return v, e
	}
	v.DisplayName, e = d.string()
	if e != nil {
		return v, e
	}
	v.Flags, e = decode_user_flags(d)
	if e != nil {
		return v, e
	}
	if v.Flags&(1<<2) != 0 {
		var guarded_avatar_url string
		guarded_avatar_url, e = d.string()
		if e != nil {
			return v, e
		}
		v.AvatarUrl = &guarded_avatar_url
	}
	v.Presence, e = decode_presence(d)
	if e != nil {
		return v, e
	}
	{
		var n uint64
		n, e = d.varint()
		if e != nil {
			return v, e
		}
		var c int
		c, e = count(n)
		if e != nil {
			return v, e
		}
		v.Roles = make([]string, c)
		for i := range v.Roles {
			v.Roles[i], e = d.string()
			if e != nil {
				return v, e
			}
		}
	}
	return v, e
}
func EncodeUser(v User) ([]byte, error) { e := wireEncoder{}; encode_user(&e, v); return e.finish() }
func DecodeUser(b []byte) (User, error) {
	d := wireDecoder{b: b}
	v, e := decode_user(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateUser(b []byte) error {
	if C.typikon_10_user_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid User wire")
	}
	return nil
}

type Attachment struct {
	Id       uint64
	Name     string
	MimeType string
	Size     uint64
}

var AttachmentCID = []byte{0x64, 0x65, 0x65, 0xb1, 0xd9, 0x53, 0x5b, 0x06}

func encode_attachment(e *wireEncoder, v Attachment) {
	e.raw(AttachmentCID)
	e.u64(v.Id)
	e.string(v.Name)
	e.string(v.MimeType)
	e.u64(v.Size)
}
func decode_attachment(d *wireDecoder) (Attachment, error) {
	var v Attachment
	if e := cid(d, AttachmentCID); e != nil {
		return v, e
	}
	var e error
	v.Id, e = d.u64()
	if e != nil {
		return v, e
	}
	v.Name, e = d.string()
	if e != nil {
		return v, e
	}
	v.MimeType, e = d.string()
	if e != nil {
		return v, e
	}
	v.Size, e = d.u64()
	if e != nil {
		return v, e
	}
	return v, e
}
func EncodeAttachment(v Attachment) ([]byte, error) {
	e := wireEncoder{}
	encode_attachment(&e, v)
	return e.finish()
}
func DecodeAttachment(b []byte) (Attachment, error) {
	d := wireDecoder{b: b}
	v, e := decode_attachment(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateAttachment(b []byte) error {
	if C.typikon_10_attachment_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid Attachment wire")
	}
	return nil
}

type Message struct {
	Id          uint64
	ChatId      uint64
	Sender      User
	Text        string
	Attachments []Attachment
	Metadata    map[string]string
}

var MessageCID = []byte{0xdf, 0xe8, 0x29, 0xa5, 0x51, 0x86, 0x1e, 0xf4}

func encode_message(e *wireEncoder, v Message) {
	e.raw(MessageCID)
	e.u64(v.Id)
	e.u64(v.ChatId)
	encode_user(e, v.Sender)
	e.string(v.Text)
	e.varint(uint64(len(v.Attachments)))
	for _, item := range v.Attachments {
		encode_attachment(e, item)
	}
	keys := make([]string, 0, len(v.Metadata))
	for k := range v.Metadata {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	e.varint(uint64(len(keys)))
	for _, k := range keys {
		e.string(k)
		e.string(v.Metadata[k])
	}
}
func decode_message(d *wireDecoder) (Message, error) {
	var v Message
	if e := cid(d, MessageCID); e != nil {
		return v, e
	}
	var e error
	v.Id, e = d.u64()
	if e != nil {
		return v, e
	}
	v.ChatId, e = d.u64()
	if e != nil {
		return v, e
	}
	v.Sender, e = decode_user(d)
	if e != nil {
		return v, e
	}
	v.Text, e = d.string()
	if e != nil {
		return v, e
	}
	{
		var n uint64
		n, e = d.varint()
		if e != nil {
			return v, e
		}
		var c int
		c, e = count(n)
		if e != nil {
			return v, e
		}
		v.Attachments = make([]Attachment, c)
		for i := range v.Attachments {
			v.Attachments[i], e = decode_attachment(d)
			if e != nil {
				return v, e
			}
		}
	}
	{
		var n uint64
		n, e = d.varint()
		if e != nil {
			return v, e
		}
		var c int
		c, e = count(n)
		if e != nil {
			return v, e
		}
		v.Metadata = make(map[string]string, c)
		for i := 0; i < c; i++ {
			k, e := d.string()
			if e != nil {
				return v, e
			}
			v.Metadata[k], e = d.string()
			if e != nil {
				return v, e
			}
		}
	}
	return v, e
}
func EncodeMessage(v Message) ([]byte, error) {
	e := wireEncoder{}
	encode_message(&e, v)
	return e.finish()
}
func DecodeMessage(b []byte) (Message, error) {
	d := wireDecoder{b: b}
	v, e := decode_message(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateMessage(b []byte) error {
	if C.typikon_10_message_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid Message wire")
	}
	return nil
}

type Update interface{ isUpdate() }
type UpdateMessageCreated struct{ Message Message }

func (UpdateMessageCreated) isUpdate() {}

type UpdateMessageEdited struct {
	ChatId    uint64
	MessageId uint64
	Text      string
}

func (UpdateMessageEdited) isUpdate() {}

type UpdateUserJoined struct {
	ChatId uint64
	User   User
}

func (UpdateUserJoined) isUpdate() {}
func encode_update(e *wireEncoder, v Update) {
	switch x := v.(type) {
	case UpdateMessageCreated:
		e.raw([]byte{0x20, 0x50, 0xae, 0x79, 0xc1, 0x93, 0x2b, 0x3a})
		encode_message(e, x.Message)
	case UpdateMessageEdited:
		e.raw([]byte{0x03, 0x60, 0xfb, 0x29, 0x95, 0x8d, 0xb3, 0x46})
		e.u64(x.ChatId)
		e.u64(x.MessageId)
		e.string(x.Text)
	case UpdateUserJoined:
		e.raw([]byte{0x75, 0x81, 0xf3, 0xd0, 0xbf, 0x40, 0x67, 0xa2})
		e.u64(x.ChatId)
		encode_user(e, x.User)
	default:
		panic("unknown variant")
	}
}
func decode_update(d *wireDecoder) (Update, error) {
	var v Update
	c, e := d.take(8)
	if e != nil {
		return nil, e
	}
	switch string(c) {
	case string([]byte{0x20, 0x50, 0xae, 0x79, 0xc1, 0x93, 0x2b, 0x3a}):
		var x UpdateMessageCreated
		x.Message, e = decode_message(d)
		if e != nil {
			return v, e
		}
		return x, e
	case string([]byte{0x03, 0x60, 0xfb, 0x29, 0x95, 0x8d, 0xb3, 0x46}):
		var x UpdateMessageEdited
		x.ChatId, e = d.u64()
		if e != nil {
			return v, e
		}
		x.MessageId, e = d.u64()
		if e != nil {
			return v, e
		}
		x.Text, e = d.string()
		if e != nil {
			return v, e
		}
		return x, e
	case string([]byte{0x75, 0x81, 0xf3, 0xd0, 0xbf, 0x40, 0x67, 0xa2}):
		var x UpdateUserJoined
		x.ChatId, e = d.u64()
		if e != nil {
			return v, e
		}
		x.User, e = decode_user(d)
		if e != nil {
			return v, e
		}
		return x, e
	default:
		return nil, fmt.Errorf("unknown constructor")
	}
}
func EncodeUpdate(v Update) ([]byte, error) {
	e := wireEncoder{}
	encode_update(&e, v)
	return e.finish()
}
func DecodeUpdate(b []byte) (Update, error) {
	d := wireDecoder{b: b}
	v, e := decode_update(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateUpdate(b []byte) error {
	if C.typikon_10_update_validate_borrowed(bridgePtr(b), C.size_t(len(b))) != 0 {
		return fmt.Errorf("invalid Update wire")
	}
	return nil
}

// UserView is a borrowed view over a Typikon User packet. Byte/string fields
// point into the caller-owned packet; keep the packet alive while using it.
type UserView struct {
	id          uint64
	username    []byte
	displayName []byte
	flags       UserFlags
	avatarURL   []byte
	hasAvatar   bool
	presence    Presence
	roles       [][]byte
}

func (v UserView) ID() uint64               { return v.id }
func (v UserView) UsernameBytes() []byte    { return v.username }
func (v UserView) DisplayNameBytes() []byte { return v.displayName }
func (v UserView) Flags() UserFlags         { return v.flags }
func (v UserView) AvatarURLBytes() []byte   { return v.avatarURL }
func (v UserView) HasAvatar() bool          { return v.hasAvatar }
func (v UserView) Presence() Presence       { return v.presence }
func (v UserView) RolesLen() int            { return len(v.roles) }
func (v UserView) RoleBytes(i int) ([]byte, bool) {
	if i < 0 || i >= len(v.roles) {
		return nil, false
	}
	return v.roles[i], true
}

func readUserView(d *wireDecoder) (UserView, error) {
	var v UserView
	if err := cid(d, UserCID); err != nil {
		return v, err
	}
	var err error
	if v.id, err = d.u64(); err != nil {
		return v, err
	}
	if v.username, err = d.bytes(); err != nil {
		return v, err
	}
	if v.displayName, err = d.bytes(); err != nil {
		return v, err
	}
	var flags uint16
	if flags, err = d.u16(); err != nil {
		return v, err
	}
	v.flags = UserFlags(flags)
	if v.flags&(1<<2) != 0 {
		v.hasAvatar = true
		if v.avatarURL, err = d.bytes(); err != nil {
			return v, err
		}
	}
	if v.presence, err = decode_presence(d); err != nil {
		return v, err
	}
	var n uint64
	if n, err = d.varint(); err != nil {
		return v, err
	}
	c, err := count(n)
	if err != nil {
		return v, err
	}
	v.roles = make([][]byte, c)
	for i := range v.roles {
		if v.roles[i], err = d.bytes(); err != nil {
			return v, err
		}
	}
	return v, nil
}

func BorrowUser(wire []byte) (UserView, error) {
	d := wireDecoder{b: wire}
	v, err := readUserView(&d)
	if err == nil {
		err = d.done()
	}
	return v, err
}

type AttachmentView struct {
	id             uint64
	name, mimeType []byte
	size           uint64
}

func (v AttachmentView) ID() uint64            { return v.id }
func (v AttachmentView) NameBytes() []byte     { return v.name }
func (v AttachmentView) MimeTypeBytes() []byte { return v.mimeType }
func (v AttachmentView) Size() uint64          { return v.size }
func readAttachmentView(d *wireDecoder) (AttachmentView, error) {
	var v AttachmentView
	if err := cid(d, AttachmentCID); err != nil {
		return v, err
	}
	var e error
	if v.id, e = d.u64(); e != nil {
		return v, e
	}
	if v.name, e = d.bytes(); e != nil {
		return v, e
	}
	if v.mimeType, e = d.bytes(); e != nil {
		return v, e
	}
	v.size, e = d.u64()
	return v, e
}

type MapEntryView struct{ Key, Value []byte }
type MessageView struct {
	id, chatID  uint64
	sender      UserView
	text        []byte
	attachments []AttachmentView
	metadata    []MapEntryView
}

func (v MessageView) ID() uint64          { return v.id }
func (v MessageView) ChatID() uint64      { return v.chatID }
func (v MessageView) Sender() UserView    { return v.sender }
func (v MessageView) TextBytes() []byte   { return v.text }
func (v MessageView) AttachmentsLen() int { return len(v.attachments) }
func (v MessageView) Attachment(i int) (AttachmentView, bool) {
	if i < 0 || i >= len(v.attachments) {
		return AttachmentView{}, false
	}
	return v.attachments[i], true
}
func (v MessageView) MetadataLen() int { return len(v.metadata) }
func (v MessageView) Metadata(i int) (MapEntryView, bool) {
	if i < 0 || i >= len(v.metadata) {
		return MapEntryView{}, false
	}
	return v.metadata[i], true
}
func readMessageView(d *wireDecoder) (MessageView, error) {
	var v MessageView
	if err := cid(d, MessageCID); err != nil {
		return v, err
	}
	var e error
	if v.id, e = d.u64(); e != nil {
		return v, e
	}
	if v.chatID, e = d.u64(); e != nil {
		return v, e
	}
	if v.sender, e = readUserView(d); e != nil {
		return v, e
	}
	if v.text, e = d.bytes(); e != nil {
		return v, e
	}
	var n uint64
	if n, e = d.varint(); e != nil {
		return v, e
	}
	c, e := count(n)
	if e != nil {
		return v, e
	}
	v.attachments = make([]AttachmentView, c)
	for i := range v.attachments {
		if v.attachments[i], e = readAttachmentView(d); e != nil {
			return v, e
		}
	}
	if n, e = d.varint(); e != nil {
		return v, e
	}
	c, e = count(n)
	if e != nil {
		return v, e
	}
	v.metadata = make([]MapEntryView, c)
	for i := range v.metadata {
		if v.metadata[i].Key, e = d.bytes(); e != nil {
			return v, e
		}
		if v.metadata[i].Value, e = d.bytes(); e != nil {
			return v, e
		}
	}
	return v, nil
}
func BorrowMessage(wire []byte) (MessageView, error) {
	d := wireDecoder{b: wire}
	v, e := readMessageView(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}

type UpdateView interface{ isUpdateView() }
type MessageCreatedView struct{ Message MessageView }

func (MessageCreatedView) isUpdateView() {}

type MessageEditedView struct {
	ChatID, MessageID uint64
	Text              []byte
}

func (MessageEditedView) isUpdateView() {}

type UserJoinedView struct {
	ChatID uint64
	User   UserView
}

func (UserJoinedView) isUpdateView() {}
func BorrowUpdate(wire []byte) (UpdateView, error) {
	d := wireDecoder{b: wire}
	c, e := d.take(8)
	if e != nil {
		return nil, e
	}
	switch string(c) {
	case string([]byte{0x20, 0x50, 0xae, 0x79, 0xc1, 0x93, 0x2b, 0x3a}):
		v, e := readMessageView(&d)
		if e != nil {
			return nil, e
		}
		if e = d.done(); e != nil {
			return nil, e
		}
		return MessageCreatedView{v}, nil
	case string([]byte{0x03, 0x60, 0xfb, 0x29, 0x95, 0x8d, 0xb3, 0x46}):
		a, e := d.u64()
		if e != nil {
			return nil, e
		}
		m, e := d.u64()
		if e != nil {
			return nil, e
		}
		t, e := d.bytes()
		if e != nil {
			return nil, e
		}
		if e = d.done(); e != nil {
			return nil, e
		}
		return MessageEditedView{a, m, t}, nil
	case string([]byte{0x75, 0x81, 0xf3, 0xd0, 0xbf, 0x40, 0x67, 0xa2}):
		a, e := d.u64()
		if e != nil {
			return nil, e
		}
		u, e := readUserView(&d)
		if e != nil {
			return nil, e
		}
		if e = d.done(); e != nil {
			return nil, e
		}
		return UserJoinedView{a, u}, nil
	default:
		return nil, fmt.Errorf("unknown Update constructor")
	}
}
