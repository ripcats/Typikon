include!(concat!(env!("OUT_DIR"), "/typescript-bridge.rs"));

#[cfg(test)]
mod tests {
    use crate::messenger_10::{Presence, User, UserFlags, UserRef};
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
}
