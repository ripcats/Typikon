#[version(10)]

#[flags(u16)]
enum UserFlags {
    IsBot = 0,
    IsVerified = 1,
    HasAvatar = 2,
}

enum Presence {
    Online = 0,
    Away = 1,
    Offline = 2,
}

struct User {
    id: u64,
    username: String,
    display_name: String,
    flags: UserFlags,

    #[guard(flags.has_avatar)]
    avatar_url: String,

    presence: Presence,
    roles: Vec<String>,
}

struct Attachment {
    id: u64,
    name: String,
    mime_type: String,
    size: u64,
}

struct Message {
    id: u64,
    chat_id: u64,
    sender: User,
    text: String,
    attachments: Vec<Attachment>,
    metadata: Map<String, String>,
}

enum Update {
    MessageCreated { message: Message },
    MessageEdited { chat_id: u64, message_id: u64, text: String },
    UserJoined { chat_id: u64, user: User },
}
