package typikon

import (
	"bytes"
	"encoding/hex"
	"testing"
)

func TestABIVersion(t *testing.T) {
	if got := ABIVersion(); got != 1 {
		t.Fatalf("ABI version = %d, want 1", got)
	}
}

func TestNegotiateLayer(t *testing.T) {
	got, err := NegotiateLayer(8, []uint16{6, 8, 10})
	if err != nil || got != 8 {
		t.Fatalf("supported Layer result = (%d, %v)", got, err)
	}
	if _, err := NegotiateLayer(9, []uint16{6, 8, 10}); err == nil {
		t.Fatal("unsupported Layer unexpectedly negotiated")
	}
}

func TestUserRoundTrip(t *testing.T) {
	want := User{
		Id:          7,
		Username:    "ada",
		DisplayName: "Ada",
		Flags:       0,
		Presence:    Presence("Online"),
		Roles:       []string{},
	}
	wire, err := EncodeUser(want)
	if err != nil {
		t.Fatalf("EncodeUser: %v", err)
	}
	wantWire, _ := hex.DecodeString("acb38da67a712058070000000000000003616461034164610000000000000000000000")
	if !bytes.Equal(wire, wantWire) {
		t.Fatalf("wire = %x, want %x", wire, wantWire)
	}
	got, err := DecodeUser(wire)
	if err != nil {
		t.Fatalf("DecodeUser: %v", err)
	}
	if got.Id != want.Id || got.Username != want.Username || got.DisplayName != want.DisplayName ||
		got.Flags != want.Flags || got.AvatarUrl != nil || got.Presence != want.Presence || len(got.Roles) != 0 {
		t.Fatalf("round-trip = %#v, want %#v", got, want)
	}
}

func TestUserDecodeRejectsInvalidWire(t *testing.T) {
	if _, err := DecodeUser([]byte{0xff}); err == nil {
		t.Fatal("invalid wire unexpectedly decoded")
	}
}

func TestUserBorrowedValidation(t *testing.T) {
	want := User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online"), Roles: []string{}}
	wire, err := EncodeUser(want)
	if err != nil {
		t.Fatalf("EncodeUser: %v", err)
	}
	if err := ValidateUser(wire); err != nil {
		t.Fatalf("ValidateUser: %v", err)
	}
	if err := ValidateUser([]byte{0xff}); err == nil {
		t.Fatal("invalid wire unexpectedly validated")
	}
}

func TestUserBorrowedViewAliasesPacket(t *testing.T) {
	wire, err := EncodeUser(User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online"), Roles: []string{"admin"}})
	if err != nil {
		t.Fatalf("EncodeUser: %v", err)
	}
	view, err := BorrowUser(wire)
	if err != nil {
		t.Fatalf("BorrowUser: %v", err)
	}
	if string(view.UsernameBytes()) != "ada" || view.RolesLen() != 1 {
		t.Fatalf("unexpected view: %#v", view)
	}
	pos := bytes.Index(wire, []byte("ada"))
	if pos < 0 {
		t.Fatal("username not found in packet")
	}
	wire[pos] = 'z'
	if string(view.UsernameBytes()) != "zda" {
		t.Fatalf("view does not alias packet: %q", view.UsernameBytes())
	}
	role, ok := view.RoleBytes(0)
	if !ok || string(role) != "admin" {
		t.Fatalf("role view = %q, %v", role, ok)
	}
}

func TestBorrowedViewsCoverNestedAndEnumPayloads(t *testing.T) {
	wire, err := EncodeMessage(Message{Id: 1, ChatId: 2, Sender: User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online"), Roles: []string{"admin"}}, Text: "hello", Attachments: []Attachment{{Id: 3, Name: "a.txt", MimeType: "text/plain", Size: 5}}, Metadata: map[string]string{"k": "v"}})
	if err != nil {
		t.Fatalf("EncodeMessage: %v", err)
	}
	message, err := BorrowMessage(wire)
	if err != nil {
		t.Fatalf("BorrowMessage: %v", err)
	}
	if string(message.TextBytes()) != "hello" || message.AttachmentsLen() != 1 || message.MetadataLen() != 1 {
		t.Fatalf("unexpected message view")
	}
	updateWire, err := EncodeUpdate(UpdateMessageEdited{ChatId: 2, MessageId: 1, Text: "edit"})
	if err != nil {
		t.Fatalf("EncodeUpdate: %v", err)
	}
	update, err := BorrowUpdate(updateWire)
	if err != nil {
		t.Fatalf("BorrowUpdate: %v", err)
	}
	edited, ok := update.(MessageEditedView)
	if !ok || string(edited.Text) != "edit" {
		t.Fatalf("unexpected update view: %#v", update)
	}
	createdWire, err := EncodeUpdate(UpdateMessageCreated{Message: Message{Sender: User{Presence: Presence("Online")}, Attachments: []Attachment{{Name: "nested"}}, Metadata: map[string]string{}}})
	if err != nil {
		t.Fatalf("EncodeUpdate created: %v", err)
	}
	created, err := BorrowUpdateLazy(createdWire)
	if err != nil {
		t.Fatalf("BorrowUpdateLazy: %v", err)
	}
	createdView, ok := created.(UpdateMessageCreatedLazyView)
	if !ok || createdView.Message.AttachmentsLen() != 1 {
		t.Fatalf("nested enum lazy view was materialized: %#v", created)
	}
	lazy, err := BorrowMessageLazy(wire)
	if err != nil {
		t.Fatalf("BorrowMessageLazy: %v", err)
	}
	if lazy.Sender().RolesLen() != 1 {
		t.Fatalf("nested lazy roles were materialized or lost")
	}
	attachment, ok := lazy.Attachment(0)
	if !ok || string(attachment.NameBytes()) != "a.txt" {
		t.Fatalf("unexpected lazy attachment")
	}
	entry, ok := lazy.Metadata(0)
	if !ok || string(entry.Key) != "k" {
		t.Fatalf("unexpected lazy metadata")
	}
}

func TestBorrowedViewsRejectUnsortedMap(t *testing.T) {
	wire, err := EncodeMessage(Message{Sender: User{Presence: Presence("Online")}, Metadata: map[string]string{"a": "1", "b": "2"}})
	if err != nil {
		t.Fatalf("EncodeMessage: %v", err)
	}
	pos := bytes.Index(wire, []byte("a"))
	if pos < 0 {
		t.Fatal("map key not found")
	}
	wire[pos] = 'z'
	if _, err := BorrowMessage(wire); err == nil {
		t.Fatal("BorrowMessage accepted unsorted map")
	}
	if _, err := BorrowMessageLazy(wire); err == nil {
		t.Fatal("BorrowMessageLazy accepted unsorted map")
	}
}

func TestAttachmentBorrowedViewAliasesPacket(t *testing.T) {
	wire, err := EncodeAttachment(Attachment{Id: 3, Name: "a.txt", MimeType: "text/plain", Size: 5})
	if err != nil {
		t.Fatalf("EncodeAttachment: %v", err)
	}
	view, err := BorrowAttachment(wire)
	if err != nil {
		t.Fatalf("BorrowAttachment: %v", err)
	}
	if view.ID() != 3 || string(view.NameBytes()) != "a.txt" || string(view.MimeTypeBytes()) != "text/plain" || view.Size() != 5 {
		t.Fatalf("unexpected attachment view")
	}
	pos := bytes.Index(wire, []byte("a.txt"))
	if pos < 0 {
		t.Fatal("attachment name not found in packet")
	}
	wire[pos] = 'b'
	if string(view.NameBytes()) != "b.txt" {
		t.Fatalf("attachment view does not alias packet: %q", view.NameBytes())
	}
}

func TestAttachmentBorrowedViewRejectsMalformedWire(t *testing.T) {
	if _, err := BorrowAttachment([]byte{0xff}); err == nil {
		t.Fatal("BorrowAttachment accepted malformed wire")
	}
}

func TestBorrowedViewsRejectMalformedWire(t *testing.T) {
	if _, err := BorrowUser([]byte{0xff}); err == nil {
		t.Fatal("BorrowUser accepted malformed wire")
	}
	if _, err := BorrowMessage([]byte{0xff}); err == nil {
		t.Fatal("BorrowMessage accepted malformed wire")
	}
	if _, err := BorrowUpdate([]byte{0xff}); err == nil {
		t.Fatal("BorrowUpdate accepted malformed wire")
	}
}
