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
