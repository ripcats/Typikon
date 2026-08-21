#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
temp_dir="$(mktemp -d /tmp/typikon-generated-go.XXXXXX)"
trap 'rm -rf "$temp_dir"' EXIT

cargo build --quiet --manifest-path "$repo_dir/bindings/go/native/Cargo.toml"
cargo run --quiet --manifest-path "$repo_dir/Cargo.toml" -- compile \
    "$repo_dir/examples/messenger.typ" --out-dir "$temp_dir/generated" --target golang
gofmt -w "$temp_dir/generated/messenger-10.go"

cp "$temp_dir/generated/messenger-10.go" "$temp_dir/generated/messenger-10.h" "$temp_dir/"
cat >"$temp_dir/go.mod" <<'EOF'
module typikon-generated-go-test

go 1.22
EOF
cat >"$temp_dir/generated_views_test.go" <<'EOF'
package typikon

import (
	"bytes"
	"testing"
)

func TestGeneratedBorrowedViews(t *testing.T) {
	wire, err := EncodeMessage(Message{
		Id: 1, ChatId: 2,
		Sender: User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online"), Roles: []string{"admin"}},
		Text: "hello",
		Attachments: []Attachment{{Id: 3, Name: "a.txt", MimeType: "text/plain", Size: 5}},
		Metadata: map[string]string{"k": "v"},
	})
	if err != nil {
		t.Fatalf("EncodeMessage: %v", err)
	}
	view, err := BorrowMessage(wire)
	if err != nil {
		t.Fatalf("BorrowMessage: %v", err)
	}
	if string(view.Text) != "hello" || string(view.Sender.Username) != "ada" || len(view.Attachments) != 1 || len(view.Metadata) != 1 {
		t.Fatalf("unexpected generated message view: %#v", view)
	}
	if string(view.Attachments[0].Name) != "a.txt" || string(view.Metadata[0].Key) != "k" {
		t.Fatalf("unexpected nested generated views")
	}

	textPos := bytes.Index(wire, []byte("hello"))
	if textPos < 0 {
		t.Fatal("message text not found in packet")
	}
	wire[textPos] = 'j'
	if string(view.Text) != "jello" {
		t.Fatalf("message text view does not alias packet: %q", view.Text)
	}
	namePos := bytes.Index(wire, []byte("a.txt"))
	if namePos < 0 {
		t.Fatal("attachment name not found in packet")
	}
	wire[namePos] = 'b'
	if string(view.Attachments[0].Name) != "b.txt" {
		t.Fatalf("attachment view does not alias packet: %q", view.Attachments[0].Name)
	}

	attachmentWire, err := EncodeAttachment(Attachment{Id: 3, Name: "a.txt", MimeType: "text/plain", Size: 5})
	if err != nil {
		t.Fatalf("EncodeAttachment: %v", err)
	}
	attachment, err := BorrowAttachment(attachmentWire)
	if err != nil || string(attachment.Name) != "a.txt" {
		t.Fatalf("BorrowAttachment: %#v, %v", attachment, err)
	}

	updateWire, err := EncodeUpdate(UpdateMessageEdited{ChatId: 2, MessageId: 1, Text: "edit"})
	if err != nil {
		t.Fatalf("EncodeUpdate: %v", err)
	}
	update, err := BorrowUpdate(updateWire)
	if err != nil {
		t.Fatalf("BorrowUpdate: %v", err)
	}
	edited, ok := update.(UpdateMessageEditedView)
	if !ok || string(edited.Text) != "edit" {
		t.Fatalf("unexpected generated enum view: %#v", update)
	}
}

func TestGeneratedBorrowedViewsRejectTruncatedWire(t *testing.T) {
	wire, err := EncodeUser(User{Id: 7, Username: "ada", DisplayName: "Ada", Presence: Presence("Online")})
	if err != nil {
		t.Fatalf("EncodeUser: %v", err)
	}
	if _, err := BorrowUser(wire[:len(wire)-1]); err == nil {
		t.Fatal("BorrowUser accepted truncated wire")
	}
}
EOF

(cd "$temp_dir" && CGO_LDFLAGS="-L$repo_dir/bindings/go/native/target/debug -Wl,-rpath,$repo_dir/bindings/go/native/target/debug -ltypikon_go_native" go test ./...)
printf 'Generated Go borrowed views: PASS\n'
