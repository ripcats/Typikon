use crate::ast::{Alias, Enum, EnumVariant, Field, Flags, FlagsBit, Item, Schema, Struct, Type};
use crate::error::ParseError;
use crate::limits::MAX_NESTING_DEPTH;

pub fn parse_schema(source: &str) -> Result<Schema, ParseError> {
    let schema = Parser {
        source,
        position: 0,
    }
    .parse_schema()?;
    crate::validate::validate(&schema)?;
    Ok(schema)
}

struct Parser<'a> {
    source: &'a str,
    position: usize,
}

impl<'a> Parser<'a> {
    fn parse_schema(mut self) -> Result<Schema, ParseError> {
        self.expect("#[version(")?;
        let version = self.number()?;
        let version = u16::try_from(version).map_err(|_| ParseError {
            message: "version must fit u16".into(),
            position: self.position,
        })?;
        self.expect(")]")?;
        let mut items = Vec::new();
        while !self.eof() {
            items.push(self.parse_item()?);
        }
        Ok(Schema { version, items })
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        let cid = self.parse_cid_attr()?;
        if self.consume("type") {
            let name = self.identifier()?;
            self.expect("=")?;
            let ty = self.parse_type()?;
            self.consume(";");
            return Ok(Item::Alias(Alias { name, ty }));
        }
        if self.consume("#[flags(") {
            let underlying = self.identifier()?;
            self.expect(")]")?;
            self.expect("enum")?;
            return Ok(Item::Flags(self.parse_flags(underlying)?));
        }
        if self.consume("struct") {
            return Ok(Item::Struct(self.parse_struct(cid)?));
        }
        if self.consume("enum") {
            return Ok(Item::Enum(self.parse_enum()?));
        }
        self.error("expected `type`, `struct`, `enum` or `#[flags(...)] enum`")
    }

    fn parse_struct(&mut self, cid: Option<String>) -> Result<Struct, ParseError> {
        let name = self.identifier()?;
        Ok(Struct {
            name,
            cid,
            fields: self.parse_fields()?,
        })
    }

    fn parse_enum(&mut self) -> Result<Enum, ParseError> {
        let name = self.identifier()?;
        self.expect("{")?;
        let mut variants = Vec::new();
        while !self.consume("}") {
            let cid = self.parse_cid_attr()?;
            let variant_name = self.identifier()?;
            let fields = if self.consume("{") {
                self.parse_fields_after_open()?
            } else {
                Vec::new()
            };
            let value = if self.consume("=") {
                Some(self.number()?)
            } else {
                None
            };
            self.consume(",");
            variants.push(EnumVariant {
                name: variant_name,
                cid,
                fields,
                value,
            });
        }
        Ok(Enum { name, variants })
    }

    fn parse_flags(&mut self, underlying: String) -> Result<Flags, ParseError> {
        let name = self.identifier()?;
        self.expect("{")?;
        let mut bits = Vec::new();
        while !self.consume("}") {
            let name = self.identifier()?;
            self.expect("=")?;
            let value = self.number()?;
            self.consume(",");
            bits.push(FlagsBit { name, value });
        }
        Ok(Flags {
            name,
            underlying,
            bits,
        })
    }

    fn parse_fields(&mut self) -> Result<Vec<Field>, ParseError> {
        self.expect("{")?;
        self.parse_fields_after_open()
    }

    fn parse_fields_after_open(&mut self) -> Result<Vec<Field>, ParseError> {
        let mut fields = Vec::new();
        loop {
            if self.consume("}") {
                break;
            }
            let guard = self.parse_guard_attr()?;
            let name = self.identifier()?;
            self.expect(":")?;
            let ty = self.parse_type()?;
            let exact_len = self.parse_exact_len_attr()?;
            fields.push(Field {
                name,
                guard,
                exact_len,
                ty,
            });
            if self.consume("}") {
                break;
            }
            self.expect(",")?;
        }
        Ok(fields)
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        self.parse_type_at(0)
    }

    fn parse_type_at(&mut self, depth: usize) -> Result<Type, ParseError> {
        if depth > MAX_NESTING_DEPTH {
            return self.error("maximum type nesting depth exceeded");
        }
        let name = self.identifier()?;
        match name.as_str() {
            "bytes" => {
                self.expect("[")?;
                let length = self.number()?;
                self.expect("]")?;
                let length = usize::try_from(length).map_err(|_| ParseError {
                    message: "fixed byte length does not fit usize".into(),
                    position: self.position,
                })?;
                Ok(Type::FixedBytes(length))
            }
            "Vec" => {
                self.expect("<")?;
                let t = self.parse_type_at(depth + 1)?;
                self.expect(">")?;
                Ok(Type::Vec(Box::new(t)))
            }
            "Map" => {
                self.expect("<")?;
                let k = self.parse_type_at(depth + 1)?;
                self.expect(",")?;
                let v = self.parse_type_at(depth + 1)?;
                self.expect(">")?;
                Ok(Type::Map(Box::new(k), Box::new(v)))
            }
            _ => Ok(Type::Primitive(name)),
        }
    }

    fn parse_cid_attr(&mut self) -> Result<Option<String>, ParseError> {
        if !self.consume("#[cid(") {
            return Ok(None);
        }
        self.skip_trivia();
        let start = self.position;
        while let Some(c) = self.source[self.position..].chars().next() {
            if c.is_ascii_hexdigit() {
                self.position += c.len_utf8();
            } else {
                break;
            }
        }
        let cid = self.source[start..self.position].to_owned();
        if cid.is_empty() {
            return self.error("expected hexadecimal C-ID");
        }
        self.expect(")]")?;
        if cid.len() != 16 || !cid.chars().all(|c| c.is_ascii_hexdigit()) {
            return self.error("C-ID must contain exactly 16 hexadecimal characters");
        }
        Ok(Some(cid.to_lowercase()))
    }

    fn parse_guard_attr(&mut self) -> Result<Option<String>, ParseError> {
        if !self.consume("#[guard(") {
            return Ok(None);
        }
        let owner = self.identifier()?;
        self.expect(".")?;
        let bit = self.identifier()?;
        self.expect(")]")?;
        Ok(Some(format!("{owner}.{bit}")))
    }

    fn parse_exact_len_attr(&mut self) -> Result<Option<usize>, ParseError> {
        if !self.consume("#[exact_len(") {
            return Ok(None);
        }
        let length = self.number()?;
        self.expect(")]")?;
        usize::try_from(length).map(Some).map_err(|_| ParseError {
            message: "exact length does not fit usize".into(),
            position: self.position,
        })
    }

    fn identifier(&mut self) -> Result<String, ParseError> {
        self.skip_trivia();
        let start = self.position;
        let first = self.source[self.position..].chars().next();
        if !matches!(first, Some(c) if c.is_ascii_alphabetic() || c == '_') {
            return self.error("expected identifier");
        }
        self.position += first.unwrap().len_utf8();
        while let Some(c) = self.source[self.position..].chars().next() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.position += c.len_utf8();
            } else {
                break;
            }
        }
        if start == self.position {
            self.error("expected identifier")
        } else {
            Ok(self.source[start..self.position].into())
        }
    }

    fn number(&mut self) -> Result<u64, ParseError> {
        self.skip_trivia();
        let start = self.position;
        while let Some(c) = self.source[self.position..].chars().next() {
            if c.is_ascii_digit() {
                self.position += c.len_utf8();
            } else {
                break;
            }
        }
        if start == self.position {
            return self.error("expected integer");
        }
        self.source[start..self.position]
            .parse()
            .map_err(|_| ParseError {
                message: "integer out of range".into(),
                position: start,
            })
    }

    fn expect(&mut self, text: &str) -> Result<(), ParseError> {
        self.skip_trivia();
        if self.source[self.position..].starts_with(text) {
            self.position += text.len();
            Ok(())
        } else {
            self.error(&format!("expected `{text}`"))
        }
    }

    fn consume(&mut self, text: &str) -> bool {
        self.skip_trivia();
        if self.source[self.position..].starts_with(text) {
            self.position += text.len();
            true
        } else {
            false
        }
    }

    fn skip_trivia(&mut self) {
        loop {
            while let Some(c) = self.source[self.position..].chars().next() {
                if c.is_whitespace() {
                    self.position += c.len_utf8();
                } else {
                    break;
                }
            }
            if self.source[self.position..].starts_with("//") {
                while self.position < self.source.len()
                    && self.source.as_bytes()[self.position] != b'\n'
                {
                    self.position += 1;
                }
            } else {
                break;
            }
        }
    }

    fn eof(&mut self) -> bool {
        self.skip_trivia();
        self.position == self.source.len()
    }
    fn error<T>(&self, message: &str) -> Result<T, ParseError> {
        Err(ParseError {
            message: message.into(),
            position: self.position,
        })
    }
}
