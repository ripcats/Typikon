#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub version: u16,
    pub items: Vec<Item>,
    pub comments: Vec<String>,
    pub item_comments: Vec<Vec<String>>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub request: String,
    pub result: String,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Alias(Alias),
    Struct(Struct),
    Enum(Enum),
    Flags(Flags),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alias {
    pub name: String,
    pub ty: Type,
    pub exact_len: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    pub name: String,
    pub cid: Option<String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub name: String,
    pub variants: Vec<EnumVariant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub cid: Option<String>,
    pub fields: Vec<Field>,
    pub value: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flags {
    pub name: String,
    pub underlying: String,
    pub bits: Vec<FlagsBit>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagsBit {
    pub name: String,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub guard: Option<String>,
    pub exact_len: Option<usize>,
    pub ty: Type,
    pub deprecated: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Primitive(String),
    FixedBytes(usize),
    Optional(Box<Type>),
    Vec(Box<Type>),
    Map(Box<Type>, Box<Type>),
}
