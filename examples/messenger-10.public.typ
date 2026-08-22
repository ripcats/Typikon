#[version(10)]

#[flags(u16)] enum UserFlags {
    IsBot = 0,
    IsVerified = 1,
    HasAvatar = 2
}

enum Presence {
    Online = 0,
    Away = 1,
    Offline = 2
}

#[cid(acb38da67a712058)]
struct User {
    id: u64,
    username: String,
    display_name: String,
    flags: UserFlags,
    #[guard(flags.has_avatar)] avatar_url: String,
    presence: Presence,
    roles: Vec<String>
}

#[cid(646565b1d9535b06)]
struct Attachment {
    id: u64,
    name: String,
    mime_type: String,
    size: u64
}

#[cid(dfe829a551861ef4)]
struct Message {
    id: u64,
    chat_id: u64,
    sender: User,
    text: String,
    attachments: Vec<Attachment>,
    metadata: Map<String, String>
}

enum Update {
    #[cid(2050ae79c1932b3a)] MessageCreated { message: Message },
    #[cid(0360fb29958db346)] MessageEdited { chat_id: u64, message_id: u64, text: String },
    #[cid(7581f3d0bf4067a2)] UserJoined { chat_id: u64, user: User }
}
