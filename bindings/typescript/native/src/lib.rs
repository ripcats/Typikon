include!(concat!(env!("OUT_DIR"), "/typescript-bridge.rs"));

#[cfg(test)]
mod tests {
    use crate::messenger_10::{
        Attachment, Message, MessageRef, Presence, User, UserFlags, UserRef,
    };
    use typikon::TypikonCodec;

    #[test]
    fn generated_user_ref_borrows_direct_strings() {
        let user = User {
            id: 7,
            username: "ada".into(),
            display_name: "Ada".into(),
            flags: UserFlags(0),
            avatar_url: None,
            presence: Presence::Online,
            roles: Vec::new(),
        };
        let wire = user.encode().unwrap();
        let borrowed = UserRef::decode_borrowed(&wire).unwrap();
        assert_eq!(borrowed.username, "ada");
        assert_eq!(borrowed.display_name, "Ada");
        let start = wire.as_ptr() as usize;
        let end = start + wire.len();
        assert!((start..end).contains(&(borrowed.username.as_ptr() as usize)));
        assert!((start..end).contains(&(borrowed.display_name.as_ptr() as usize)));
    }

    #[test]
    fn generated_nested_ref_reuses_the_same_packet_storage() {
        let message = Message {
            id: 1,
            chat_id: 2,
            sender: User {
                id: 7,
                username: "ada".into(),
                display_name: "Ada".into(),
                flags: UserFlags(0),
                avatar_url: None,
                presence: Presence::Online,
                roles: vec!["admin".into(), "mod".into()],
            },
            text: "hello".into(),
            attachments: vec![Attachment {
                id: 3,
                name: "photo".into(),
                mime_type: "image/jpeg".into(),
                size: 42,
            }],
            metadata: std::collections::BTreeMap::from([("source".into(), "web".into())]),
        };
        let wire = message.encode().unwrap();
        let borrowed = MessageRef::decode_borrowed(&wire).unwrap();
        assert_eq!(borrowed.sender.username, "ada");
        assert_eq!(borrowed.text, "hello");
        let role = borrowed.sender.roles.iter().next().unwrap().unwrap();
        assert_eq!(role, "admin");
        let attachment = borrowed.attachments.iter().next().unwrap().unwrap();
        assert_eq!(attachment.name, "photo");
        let (key, value) = borrowed.metadata.iter().next().unwrap().unwrap();
        assert_eq!((key, value), ("source", "web"));
        let start = wire.as_ptr() as usize;
        let end = start + wire.len();
        assert!((start..end).contains(&(borrowed.sender.username.as_ptr() as usize)));
        assert!((start..end).contains(&(borrowed.text.as_ptr() as usize)));
        assert!((start..end).contains(&(role.as_ptr() as usize)));
        assert!((start..end).contains(&(attachment.name.as_ptr() as usize)));
        assert!((start..end).contains(&(key.as_ptr() as usize)));
        assert!((start..end).contains(&(value.as_ptr() as usize)));
    }
}
