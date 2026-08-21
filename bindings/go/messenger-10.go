package typikon

import (
	"encoding/binary"
	"fmt"
	"math"
	"sort"
)

const (
	maxPacketSize = 4 << 20
	maxItems      = 1_000_000
)

type wireEncoder struct{ b []byte }

func (e *wireEncoder) raw(v []byte) { e.b = append(e.b, v...) }
func (e *wireEncoder) u64(v uint64) {
	var x [8]byte
	binary.LittleEndian.PutUint64(x[:], v)
	e.raw(x[:])
}
func (e *wireEncoder) u16(v uint16) {
	var x [2]byte
	binary.LittleEndian.PutUint16(x[:], v)
	e.raw(x[:])
}
func (e *wireEncoder) i64(v int64)   { e.u64(uint64(v)) }
func (e *wireEncoder) f64(v float64) { e.u64(math.Float64bits(v)) }
func (e *wireEncoder) varint(v uint64) {
	for v >= 0x80 {
		e.b = append(e.b, byte(v)|0x80)
		v >>= 7
	}
	e.b = append(e.b, byte(v))
}
func (e *wireEncoder) bytes(v []byte)  { e.varint(uint64(len(v))); e.raw(v) }
func (e *wireEncoder) string(v string) { e.bytes([]byte(v)) }
func (e *wireEncoder) finish() ([]byte, error) {
	if len(e.b) > maxPacketSize {
		return nil, fmt.Errorf("packet exceeds %d bytes", maxPacketSize)
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
func (d *wireDecoder) raw(n int) ([]byte, error) { return d.take(n) }
func (d *wireDecoder) u64() (uint64, error) {
	v, err := d.take(8)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint64(v), nil
}
func (d *wireDecoder) u16() (uint16, error) {
	v, err := d.take(2)
	if err != nil {
		return 0, err
	}
	return binary.LittleEndian.Uint16(v), nil
}
func (d *wireDecoder) varint() (uint64, error) {
	var v uint64
	for i := 0; i < 10; i++ {
		b, err := d.take(1)
		if err != nil {
			return 0, err
		}
		if i == 9 && b[0] > 1 {
			return 0, fmt.Errorf("invalid varint")
		}
		v |= uint64(b[0]&0x7f) << (7 * i)
		if b[0] < 0x80 {
			return v, nil
		}
	}
	return 0, fmt.Errorf("varint overflow")
}
func (d *wireDecoder) bytes() ([]byte, error) {
	n, err := d.varint()
	if err != nil || n > maxPacketSize || n > uint64(len(d.b)-d.p) {
		return nil, fmt.Errorf("invalid byte field")
	}
	return d.take(int(n))
}
func (d *wireDecoder) string() (string, error) { v, err := d.bytes(); return string(v), err }
func (d *wireDecoder) done() error {
	if d.p != len(d.b) {
		return fmt.Errorf("trailing bytes")
	}
	return nil
}
func cid(d *wireDecoder, want []byte) error {
	got, err := d.raw(8)
	if err != nil || string(got) != string(want) {
		return fmt.Errorf("invalid constructor ID")
	}
	return nil
}
func count(n uint64) (int, error) {
	if n > maxItems || n > uint64(^uint(0)>>1) {
		return 0, fmt.Errorf("collection too large")
	}
	return int(n), nil
}

type UserFlags uint16
type Presence string
type User struct {
	Id          uint64
	Username    string
	DisplayName string
	Flags       UserFlags
	AvatarUrl   *string
	Presence    Presence
	Roles       []string
}
type Attachment struct {
	Id       uint64
	Name     string
	MimeType string
	Size     uint64
}
type Message struct {
	Id          uint64
	ChatId      uint64
	Sender      User
	Text        string
	Attachments []Attachment
	Metadata    map[string]string
}
type Update interface{ isUpdate() }
type MessageCreated struct{ Message Message }

func (MessageCreated) isUpdate() {}

type MessageEdited struct {
	ChatId    uint64
	MessageId uint64
	Text      string
}

func (MessageEdited) isUpdate() {}

type UserJoined struct {
	ChatId uint64
	User   User
}

func (UserJoined) isUpdate() {}

var userCID = [...]byte{0xac, 0xb3, 0x8d, 0xa6, 0x7a, 0x71, 0x20, 0x58}
var attachmentCID = [...]byte{0x64, 0x65, 0x65, 0xb1, 0xd9, 0x53, 0x5b, 0x06}
var messageCID = [...]byte{0xdf, 0xe8, 0x29, 0xa5, 0x51, 0x86, 0x1e, 0xf4}
var updateMessageCreatedCID = [...]byte{0x20, 0x50, 0xae, 0x79, 0xc1, 0x93, 0x2b, 0x3a}
var updateMessageEditedCID = [...]byte{0x03, 0x60, 0xfb, 0x29, 0x95, 0x8d, 0xb3, 0x46}
var updateUserJoinedCID = [...]byte{0x75, 0x81, 0xf3, 0xd0, 0xbf, 0x40, 0x67, 0xa2}

func encodePresence(e *wireEncoder, v Presence) {
	e.u64(map[Presence]uint64{"Online": 0, "Away": 1, "Offline": 2}[v])
}
func decodePresence(d *wireDecoder) (Presence, error) {
	v, err := d.u64()
	if err != nil {
		return "", err
	}
	p := []Presence{"Online", "Away", "Offline"}
	if v >= uint64(len(p)) {
		return "", fmt.Errorf("invalid Presence")
	}
	return p[v], nil
}
func encodeUser(e *wireEncoder, v User) {
	e.raw(userCID[:])
	e.u64(v.Id)
	e.string(v.Username)
	e.string(v.DisplayName)
	e.u16(uint16(v.Flags))
	if v.Flags&(1<<2) != 0 {
		e.string(*v.AvatarUrl)
	}
	encodePresence(e, v.Presence)
	e.varint(uint64(len(v.Roles)))
	for _, x := range v.Roles {
		e.string(x)
	}
}
func decodeUser(d *wireDecoder) (User, error) {
	var v User
	if err := cid(d, userCID[:]); err != nil {
		return v, err
	}
	x, err := d.u64()
	if err != nil {
		return v, err
	}
	v.Id = x
	if v.Username, err = d.string(); err != nil {
		return v, err
	}
	if v.DisplayName, err = d.string(); err != nil {
		return v, err
	}
	var flags uint16
	flags, err = d.u16()
	if err != nil {
		return v, err
	}
	v.Flags = UserFlags(flags)
	if v.Flags&(1<<2) != 0 {
		x, err := d.string()
		if err != nil {
			return v, err
		}
		v.AvatarUrl = &x
	}
	p, err := decodePresence(d)
	if err != nil {
		return v, err
	}
	v.Presence = p
	n, err := d.varint()
	if err != nil {
		return v, err
	}
	c, err := count(n)
	if err != nil {
		return v, err
	}
	v.Roles = make([]string, c)
	for i := range v.Roles {
		v.Roles[i], err = d.string()
		if err != nil {
			return v, err
		}
	}
	return v, nil
}
func encodeAttachment(e *wireEncoder, v Attachment) {
	e.raw(attachmentCID[:])
	e.u64(v.Id)
	e.string(v.Name)
	e.string(v.MimeType)
	e.u64(v.Size)
}
func decodeAttachment(d *wireDecoder) (Attachment, error) {
	var v Attachment
	if err := cid(d, attachmentCID[:]); err != nil {
		return v, err
	}
	var err error
	if v.Id, err = d.u64(); err != nil {
		return v, err
	}
	if v.Name, err = d.string(); err != nil {
		return v, err
	}
	if v.MimeType, err = d.string(); err != nil {
		return v, err
	}
	v.Size, err = d.u64()
	return v, err
}
func encodeMessage(e *wireEncoder, v Message) {
	e.raw(messageCID[:])
	e.u64(v.Id)
	e.u64(v.ChatId)
	encodeUser(e, v.Sender)
	e.string(v.Text)
	e.varint(uint64(len(v.Attachments)))
	for _, x := range v.Attachments {
		encodeAttachment(e, x)
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
func decodeMessage(d *wireDecoder) (Message, error) {
	var v Message
	if err := cid(d, messageCID[:]); err != nil {
		return v, err
	}
	var err error
	if v.Id, err = d.u64(); err != nil {
		return v, err
	}
	if v.ChatId, err = d.u64(); err != nil {
		return v, err
	}
	if v.Sender, err = decodeUser(d); err != nil {
		return v, err
	}
	if v.Text, err = d.string(); err != nil {
		return v, err
	}
	n, err := d.varint()
	if err != nil {
		return v, err
	}
	c, err := count(n)
	if err != nil {
		return v, err
	}
	v.Attachments = make([]Attachment, c)
	for i := range v.Attachments {
		v.Attachments[i], err = decodeAttachment(d)
		if err != nil {
			return v, err
		}
	}
	n, err = d.varint()
	if err != nil {
		return v, err
	}
	c, err = count(n)
	if err != nil {
		return v, err
	}
	v.Metadata = make(map[string]string, c)
	for i := 0; i < c; i++ {
		k, e := d.string()
		if e != nil {
			return v, e
		}
		x, e := d.string()
		if e != nil {
			return v, e
		}
		v.Metadata[k] = x
	}
	return v, nil
}
func EncodeUserFlags(v UserFlags) ([]byte, error) {
	e := wireEncoder{}
	e.u64(uint64(v))
	return e.finish()
}
func DecodeUserFlags(b []byte) (UserFlags, error) {
	d := wireDecoder{b: b}
	v, e := d.u64()
	if e == nil {
		e = d.done()
	}
	return UserFlags(v), e
}
func ValidateUserFlags(b []byte) error { _, e := DecodeUserFlags(b); return e }
func EncodePresence(v Presence) ([]byte, error) {
	e := wireEncoder{}
	encodePresence(&e, v)
	return e.finish()
}
func DecodePresence(b []byte) (Presence, error) {
	d := wireDecoder{b: b}
	v, e := decodePresence(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidatePresence(b []byte) error   { _, e := DecodePresence(b); return e }
func EncodeUser(v User) ([]byte, error) { e := wireEncoder{}; encodeUser(&e, v); return e.finish() }
func DecodeUser(b []byte) (User, error) {
	d := wireDecoder{b: b}
	v, e := decodeUser(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateUser(b []byte) error { _, e := DecodeUser(b); return e }
func EncodeAttachment(v Attachment) ([]byte, error) {
	e := wireEncoder{}
	encodeAttachment(&e, v)
	return e.finish()
}
func DecodeAttachment(b []byte) (Attachment, error) {
	d := wireDecoder{b: b}
	v, e := decodeAttachment(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateAttachment(b []byte) error { _, e := DecodeAttachment(b); return e }
func EncodeMessage(v Message) ([]byte, error) {
	e := wireEncoder{}
	encodeMessage(&e, v)
	return e.finish()
}
func DecodeMessage(b []byte) (Message, error) {
	d := wireDecoder{b: b}
	v, e := decodeMessage(&d)
	if e == nil {
		e = d.done()
	}
	return v, e
}
func ValidateMessage(b []byte) error { _, e := DecodeMessage(b); return e }
func EncodeUpdate(v Update) ([]byte, error) {
	e := wireEncoder{}
	switch x := v.(type) {
	case MessageCreated:
		e.raw(updateMessageCreatedCID[:])
		encodeMessage(&e, x.Message)
	case MessageEdited:
		e.raw(updateMessageEditedCID[:])
		e.u64(x.ChatId)
		e.u64(x.MessageId)
		e.string(x.Text)
	case UserJoined:
		e.raw(updateUserJoinedCID[:])
		e.u64(x.ChatId)
		encodeUser(&e, x.User)
	default:
		return nil, fmt.Errorf("unknown Update variant")
	}
	return e.finish()
}
func DecodeUpdate(b []byte) (Update, error) {
	d := wireDecoder{b: b}
	c, e := d.raw(8)
	if e != nil {
		return nil, e
	}
	switch string(c) {
	case string(updateMessageCreatedCID[:]):
		v, e := decodeMessage(&d)
		if e != nil {
			return nil, e
		}
		return finishUpdate(d, MessageCreated{v})
	case string(updateMessageEditedCID[:]):
		a, e := d.u64()
		if e != nil {
			return nil, e
		}
		m, e := d.u64()
		if e != nil {
			return nil, e
		}
		t, e := d.string()
		if e != nil {
			return nil, e
		}
		return finishUpdate(d, MessageEdited{a, m, t})
	case string(updateUserJoinedCID[:]):
		a, e := d.u64()
		if e != nil {
			return nil, e
		}
		u, e := decodeUser(&d)
		if e != nil {
			return nil, e
		}
		return finishUpdate(d, UserJoined{a, u})
	default:
		return nil, fmt.Errorf("unknown Update constructor")
	}
}
func finishUpdate(d wireDecoder, v Update) (Update, error) { return v, d.done() }
func ValidateUpdate(b []byte) error                        { _, e := DecodeUpdate(b); return e }
