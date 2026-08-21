#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Schema {
    pub version: u16,
    pub items: Vec<Item>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    Struct(Struct),
    Enum(Enum),
    Flags(Flags),
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
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Primitive(String),
    Vec(Box<Type>),
    Map(Box<Type>, Box<Type>),
}
