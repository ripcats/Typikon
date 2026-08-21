#[version(8)]

#[flags(u8)]
enum SessionFlags {
    IsSecure = 0,
}

struct Ping {
    id: u64,
    flags: SessionFlags,
}
