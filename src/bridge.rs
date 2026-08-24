use crate::codegen::{borrowed_view_name, fixed_byte_length};
use crate::fingerprint::{constructor_cid_with_schema, variant_cid_with_schema};
use crate::{Item, Schema, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeKind {
    Python,
    Go,
    TypeScript,
}

impl BridgeKind {
    pub const ALL: [Self; 3] = [Self::Python, Self::Go, Self::TypeScript];

    pub fn file_name(self, schema_name: &str, layer: u16) -> String {
        let name = match self {
            Self::Python => "python",
            Self::Go => "golang",
            Self::TypeScript => "typescript",
        };
        format!("{name}.{schema_name}-{layer}.rs")
    }
}

pub fn generate_c_header(schema: &Schema) -> String {
    let mut output = String::from(
        "#ifndef TYPIKON_LAYER_BRIDGE_H\n#define TYPIKON_LAYER_BRIDGE_H\n#include <stddef.h>\n#include <stdint.h>\n\ntypedef struct { int32_t status; uint8_t *data_ptr; size_t data_len; size_t data_capacity; uint8_t *error_ptr; size_t error_len; size_t error_capacity; } TypikonBridgeResult;\n\nvoid typikon_free_bytes(uint8_t *ptr, size_t len, size_t capacity);\n\n",
    );
    for item in &schema.items {
        let name = match item {
            Item::Alias(item) => &item.name,
            Item::Struct(item) => &item.name,
            Item::Enum(item) => &item.name,
            Item::Flags(item) => &item.name,
        };
        let function_name = snake_case(name);
        output.push_str(&format!(
            "int32_t typikon_{}_{}_validate_borrowed(const uint8_t *input, size_t len);\n",
            schema.version, function_name
        ));
    }
    output.push_str("\n#endif\n");
    output
}

pub fn generate_go_binding(schema: &Schema, header_name: &str) -> String {
    generate_go_binding_direct(schema, header_name)
}

fn generate_go_binding_direct(schema: &Schema, header_name: &str) -> String {
    let mut output = String::from(
        r#"package typikon

/*
#cgo CFLAGS: -I.
#include "__TYPIKON_HEADER__"
*/
import "C"

import (
    "encoding/binary"
    "fmt"
    "math"
    "sort"
    "unsafe"
)

const maxPacketSize = 4 << 20
const maxItems = 1_000_000
type wireEncoder struct { b []byte }
func (e *wireEncoder) raw(v []byte) { e.b = append(e.b, v...) }
func (e *wireEncoder) u8(v uint8) { e.b = append(e.b, v) }
func (e *wireEncoder) u16(v uint16) { var x [2]byte; binary.LittleEndian.PutUint16(x[:], v); e.raw(x[:]) }
func (e *wireEncoder) u32(v uint32) { var x [4]byte; binary.LittleEndian.PutUint32(x[:], v); e.raw(x[:]) }
func (e *wireEncoder) u64(v uint64) { var x [8]byte; binary.LittleEndian.PutUint64(x[:], v); e.raw(x[:]) }
func (e *wireEncoder) i8(v int8) { e.u8(uint8(v)) }
func (e *wireEncoder) i16(v int16) { e.u16(uint16(v)) }
func (e *wireEncoder) i32(v int32) { e.u32(uint32(v)) }
func (e *wireEncoder) i64(v int64) { e.u64(uint64(v)) }
func (e *wireEncoder) bool(v bool) { if v { e.u8(1) } else { e.u8(0) } }
func (e *wireEncoder) f32(v float32) { e.u32(math.Float32bits(v)) }
func (e *wireEncoder) f64(v float64) { e.u64(math.Float64bits(v)) }
func (e *wireEncoder) varint(v uint64) { for v >= 0x80 { e.u8(byte(v)|0x80); v >>= 7 }; e.u8(byte(v)) }
func (e *wireEncoder) bytes(v []byte) { e.varint(uint64(len(v))); e.raw(v) }
func (e *wireEncoder) string(v string) { e.bytes([]byte(v)) }
func (e *wireEncoder) finish() ([]byte,error) { if len(e.b)>maxPacketSize { return nil,fmt.Errorf("packet exceeds limit") }; return e.b,nil }
type wireDecoder struct { b []byte; p int }
func (d *wireDecoder) take(n int) ([]byte,error) { if n<0 || d.p>len(d.b)-n { return nil,fmt.Errorf("truncated wire") }; v:=d.b[d.p:d.p+n]; d.p+=n; return v,nil }
func (d *wireDecoder) u8()(uint8,error){v,e:=d.take(1);if e!=nil{return 0,e};return v[0],nil}
func (d *wireDecoder) u16()(uint16,error){v,e:=d.take(2);if e!=nil{return 0,e};return binary.LittleEndian.Uint16(v),nil}
func (d *wireDecoder) u32()(uint32,error){v,e:=d.take(4);if e!=nil{return 0,e};return binary.LittleEndian.Uint32(v),nil}
func (d *wireDecoder) u64()(uint64,error){v,e:=d.take(8);if e!=nil{return 0,e};return binary.LittleEndian.Uint64(v),nil}
func (d *wireDecoder) i8()(int8,error){v,e:=d.u8();return int8(v),e}
func (d *wireDecoder) i16()(int16,error){v,e:=d.u16();return int16(v),e}
func (d *wireDecoder) i32()(int32,error){v,e:=d.u32();return int32(v),e}
func (d *wireDecoder) i64()(int64,error){v,e:=d.u64();return int64(v),e}
func (d *wireDecoder) bool()(bool,error){v,e:=d.u8();return v!=0,e}
func (d *wireDecoder) f32()(float32,error){v,e:=d.u32();return math.Float32frombits(v),e}
func (d *wireDecoder) f64()(float64,error){v,e:=d.u64();return math.Float64frombits(v),e}
func (d *wireDecoder) varint()(uint64,error){var v uint64;for i:=0;i<10;i++{b,e:=d.u8();if e!=nil{return 0,e};if i==9&&b>1{return 0,fmt.Errorf("invalid varint")};v|=uint64(b&0x7f)<<(7*i);if b<0x80{return v,nil}};return 0,fmt.Errorf("varint overflow")}
func (d *wireDecoder) bytes()([]byte,error){n,e:=d.varint();if e!=nil||n>maxPacketSize||n>uint64(len(d.b)-d.p){return nil,fmt.Errorf("invalid byte field")};return d.take(int(n))}
func (d *wireDecoder) string()(string,error){v,e:=d.bytes();return string(v),e}
func (d *wireDecoder) done()error{if d.p!=len(d.b){return fmt.Errorf("trailing bytes")};return nil}
func count(n uint64)(int,error){if n>maxItems||n>uint64(^uint(0)>>1){return 0,fmt.Errorf("collection too large")};return int(n),nil}
func cid(d *wireDecoder,want []byte)error{got,e:=d.take(8);if e!=nil||string(got)!=string(want){return fmt.Errorf("invalid constructor ID")};return nil}
func bridgePtr(data []byte) *C.uint8_t { if len(data)==0 { return nil }; return (*C.uint8_t)(unsafe.Pointer(&data[0])) }

"#,
    );
    output = output.replace("__TYPIKON_HEADER__", header_name);
    output.push_str("func wireBytesCompare(a,b []byte) int { for i:=0;i<len(a)&&i<len(b);i++ { if a[i]<b[i] { return -1 }; if a[i]>b[i] { return 1 } }; if len(a)<len(b) { return -1 }; if len(a)>len(b) { return 1 }; return 0 }\n");
    for item in &schema.items {
        generate_go_item(item, schema, &mut output);
    }
    output
}

fn generate_go_item(item: &Item, schema: &Schema, output: &mut String) {
    let name = item_name(item);
    let function_name = snake_case(name);
    match item {
        Item::Alias(alias) => {
            output.push_str(&format!("type {name} = {};\n", go_type(&alias.ty, schema)));
            let mut encode = String::new();
            if let Some(length) = alias.exact_len {
                encode.push_str(&format!(
                    "if len(v)!={length}{{panic(\"invalid exact length\")}};"
                ));
            }
            go_encode_go_type(&alias.ty, "v", schema, &mut encode);
            let mut decode = String::new();
            go_decode_go_type(&alias.ty, "v", schema, &mut decode, false);
            if let Some(length) = alias.exact_len {
                decode.push_str(&format!(
                    "if len(v)!={length}{{return v,fmt.Errorf(\"invalid exact length\")}};"
                ));
            }
            output.push_str(&format!(
                "func encode_{function_name}(e *wireEncoder,v {name}) {{{encode}}}\nfunc decode_{function_name}(d *wireDecoder) ({name},error) {{ var v {name}; var e error; {decode} return v,e }}\n"
            ));
        }
        Item::Flags(flags) => {
            output.push_str(&format!(
                "type {name} {}\n",
                go_primitive(&flags.underlying)
            ));
            output.push_str(&format!(
                "func encode_{function_name}(e *wireEncoder,v {name}) {{ e.{}({}(v)) }}\n",
                go_wire_method(&flags.underlying),
                go_primitive(&flags.underlying)
            ));
            output.push_str(&format!("func decode_{function_name}(d *wireDecoder) ({name},error) {{ v,e:=d.{}();return {name}(v),e }}\n", go_wire_method(&flags.underlying)));
        }
        Item::Enum(en) if en.variants.iter().all(|v| v.fields.is_empty()) => {
            output.push_str(&format!("type {name} string\n"));
            output.push_str(&format!(
                "func encode_{function_name}(e *wireEncoder,v {name}) {{ switch v {{"
            ));
            for v in &en.variants {
                output.push_str(&format!(
                    "case \"{}\": e.u64({});",
                    v.name,
                    v.value.unwrap_or_default()
                ));
            }
            output.push_str("} }\n");
            output.push_str(&format!("func decode_{function_name}(d *wireDecoder) ({name},error) {{ v,e:=d.u64();if e!=nil{{return \"\",e}};switch v {{"));
            for v in &en.variants {
                output.push_str(&format!(
                    "case {}:return \"{}\",nil;",
                    v.value.unwrap_or_default(),
                    v.name
                ));
            }
            output.push_str(&format!(
                "}};return \"\",fmt.Errorf(\"invalid {name}\") }}\n"
            ));
        }
        Item::Struct(st) => generate_go_struct(st, schema, output),
        Item::Enum(en) => generate_go_enum(en, schema, output),
    }
    output.push_str(&format!("func Encode{name}(v {name})([]byte,error){{e:=wireEncoder{{}};encode_{function_name}(&e,v);return e.finish()}}\nfunc Decode{name}(b []byte)({name},error){{d:=wireDecoder{{b:b}};v,e:=decode_{function_name}(&d);if e==nil{{e=d.done()}};return v,e}}\nfunc Validate{name}(b []byte)error{{if C.typikon_{}_{}_validate_borrowed(bridgePtr(b),C.size_t(len(b)))!=0{{return fmt.Errorf(\"invalid {} wire\")}};return nil}}\n\n", schema.version, function_name, name));
    generate_go_view_item(item, schema, output);
}

fn go_view_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Optional(item) => format!("*{}", go_view_type(item, schema)),
        Type::FixedBytes(_) => "[]byte".into(),
        Type::Primitive(name) if name == "String" => "[]byte".into(),
        Type::Vec(_) if is_bytes_type(ty) => "[]byte".into(),
        Type::Primitive(name) if schema.items.iter().any(|i| item_name(i) == name) => {
            if go_named_view(name, schema) {
                format!("{}View", name)
            } else {
                go_type(ty, schema)
            }
        }
        Type::Vec(item) => format!("[]{}", go_view_type(item, schema)),
        Type::Map(key, value) => format!(
            "[]struct {{ Key {}; Value {} }}",
            go_view_type(key, schema),
            go_view_type(value, schema)
        ),
        Type::Primitive(_) => go_type(ty, schema),
    }
}

fn go_named_view(name: &str, schema: &Schema) -> bool {
    match schema.items.iter().find(|i| item_name(i) == name) {
        Some(Item::Struct(_)) => true,
        Some(Item::Enum(en)) => !en.variants.iter().all(|v| v.fields.is_empty()),
        _ => false,
    }
}

fn generate_go_view_item(item: &Item, schema: &Schema, output: &mut String) {
    match item {
        Item::Alias(_) => {}
        Item::Struct(st) => generate_go_view_struct(st, schema, output),
        Item::Enum(en) if !en.variants.iter().all(|v| v.fields.is_empty()) => {
            generate_go_view_enum(en, schema, output)
        }
        _ => {}
    }
}

fn generate_go_view_struct(item: &crate::Struct, schema: &Schema, output: &mut String) {
    let name = &item.name;
    output.push_str(&format!("type {name}View struct {{"));
    for field in &item.fields {
        output.push_str(&format!(
            " {} {};",
            pascal_case(&field.name),
            go_view_type(&field.ty, schema)
        ));
    }
    output.push_str("}\n");
    output.push_str(&format!("func read{}View(d *wireDecoder) ({name}View,error) {{ var v {name}View; if e:=cid(d,{name}CID);e!=nil{{return v,e}}; var e error;", name));
    for field in &item.fields {
        let lhs = format!("v.{}", pascal_case(&field.name));
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!(
                "if v.{}&(1<<{})!=0{{",
                pascal_case(owner),
                flag_value(item, bit)
            ));
            go_decode_view_type(&field.ty, &lhs, schema, output);
            output.push_str("};");
        } else {
            go_decode_view_type(&field.ty, &lhs, schema, output);
        }
    }
    output.push_str(&format!("return v,e}}\nfunc Borrow{name}(b []byte) ({name}View,error) {{ d:=wireDecoder{{b:b}}; v,e:=read{name}View(&d); if e==nil{{e=d.done()}}; return v,e }}\n\n"));
    generate_go_lazy_view_struct(item, schema, output);
}

fn go_lazy_field_name(parent: &str, field: &str) -> String {
    format!("{}{}LazyView", parent, pascal_case(field))
}

fn go_lazy_item_name(parent: &str, field: &str) -> String {
    format!("read{}{}LazyItem", parent, pascal_case(field))
}

fn go_lazy_enum_field_name(enum_name: &str, variant: &str, field: &str) -> String {
    format!(
        "{}{}{}LazyView",
        enum_name,
        pascal_case(variant),
        pascal_case(field)
    )
}

fn go_lazy_enum_item_name(enum_name: &str, variant: &str, field: &str) -> String {
    format!(
        "read{}{}{}LazyItem",
        enum_name,
        pascal_case(variant),
        pascal_case(field)
    )
}

fn go_has_lazy_view(name: &str, schema: &Schema) -> bool {
    schema
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(st) if st.name == name => Some(st.fields.iter().any(|field| {
                matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty)
            })),
            _ => None,
        })
        .unwrap_or(false)
}

fn go_lazy_view_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::FixedBytes(_) => "[]byte".into(),
        Type::Primitive(name)
            if schema.items.iter().any(|item| item_name(item) == name)
                && go_has_lazy_view(name, schema) =>
        {
            format!("{}LazyView", name)
        }
        Type::Vec(item) => format!("[]{}", go_lazy_view_type(item, schema)),
        Type::Map(key, value) => format!(
            "[]struct {{ Key {}; Value {} }}",
            go_lazy_view_type(key, schema),
            go_lazy_view_type(value, schema)
        ),
        _ => go_view_type(ty, schema),
    }
}

fn go_decode_lazy_view_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(
            "{lhs},e=d.take({length});if e!=nil{{return v,e}};"
        ));
        return;
    }
    if is_bytes_type(ty) || matches!(ty, Type::Primitive(name) if name == "String") {
        out.push_str(&format!("{lhs},e=d.bytes();if e!=nil{{return v,e}};"));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(&format!(
                "{{tag,e:=d.u8();if e!=nil{{return v,e}};if tag==1{{var x {};",
                go_lazy_view_type(item, schema)
            ));
            go_decode_lazy_view_type(item, "x", schema, out);
            out.push_str(&format!(
                "{}=&x}}else if tag!=0{{return v,fmt.Errorf(\"invalid optional marker\")}}}};",
                lhs
            ));
        }
        Type::Primitive(name) if schema.items.iter().any(|item| item_name(item) == name) => {
            if go_has_lazy_view(name, schema) {
                out.push_str(&format!(
                    "{lhs},e=read{}LazyView(d);if e!=nil{{return v,e}};",
                    name
                ));
            } else if go_named_view(name, schema) {
                out.push_str(&format!(
                    "{lhs},e=read{}View(d);if e!=nil{{return v,e}};",
                    name
                ));
            } else {
                out.push_str(&format!(
                    "{lhs},e=decode_{}(d);if e!=nil{{return v,e}};",
                    snake_case(name)
                ));
            }
        }
        Type::Primitive(name) => out.push_str(&format!(
            "{lhs},e=d.{}();if e!=nil{{return v,e}};",
            go_wire_method(name)
        )),
        Type::Vec(item) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make({},c);for i:=range {lhs}{{", go_lazy_view_type(ty, schema)));
            go_decode_lazy_view_type(item, &format!("{lhs}[i]"), schema, out);
            out.push_str("}};");
        }
        Type::Map(key, value) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make({},c);for i:=range {lhs}{{", go_lazy_view_type(ty, schema)));
            go_decode_lazy_view_type(key, &format!("{lhs}[i].Key"), schema, out);
            go_decode_lazy_view_type(value, &format!("{lhs}[i].Value"), schema, out);
            out.push_str("}};");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn generate_go_lazy_view_struct(item: &crate::Struct, schema: &Schema, output: &mut String) {
    let collections = item
        .fields
        .iter()
        .filter(|field| {
            matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty)
        })
        .collect::<Vec<_>>();
    if collections.is_empty() {
        return;
    }
    let name = &item.name;
    for field in &collections {
        let collection_name = go_lazy_field_name(name, &field.name);
        let item_name = go_lazy_item_name(name, &field.name);
        let (item_ty, decode_ty) = match &field.ty {
            Type::Vec(ty) => (go_lazy_view_type(ty, schema), ty.as_ref().clone()),
            Type::Map(key, value) => (
                format!("{}{}Entry", name, pascal_case(&field.name)),
                Type::Map(key.clone(), value.clone()),
            ),
            _ => unreachable!(),
        };
        if let Type::Map(key, value) = &field.ty {
            output.push_str(&format!(
                "type {} struct{{ Key {}; Value {} }}\n",
                item_ty,
                go_lazy_view_type(key, schema),
                go_lazy_view_type(value, schema)
            ));
        }
        output.push_str(&format!("type {collection_name} struct{{ wire []byte; start,count int }}\nfunc (v {collection_name}) Len() int {{ return v.count }}\n"));
        output.push_str(&format!(
            "func {item_name}(d *wireDecoder) ({item_ty},error) {{ var v {item_ty}; var e error;"
        ));
        match decode_ty {
            Type::Map(key, value) => {
                go_decode_lazy_view_type(&key, "v.Key", schema, output);
                go_decode_lazy_view_type(&value, "v.Value", schema, output);
            }
            ty => go_decode_lazy_view_type(&ty, "v", schema, output),
        }
        output.push_str("return v,e}\n");
        output.push_str(&format!("func (v {collection_name}) At(i int) ({item_ty},bool) {{ var zero {item_ty}; if i<0||i>=v.count{{return zero,false}}; d:=wireDecoder{{b:v.wire,p:v.start}}; var value {item_ty}; var e error; for n:=0;n<=i;n++{{value,e={item_name}(&d);if e!=nil{{return zero,false}}}}; return value,true }}\n"));
        output.push_str(&format!("type {collection_name}Iter struct{{ view {collection_name}; index int; decoder wireDecoder }}\nfunc (v {collection_name}) Iter() *{collection_name}Iter {{ return &{collection_name}Iter{{view:v,decoder:wireDecoder{{b:v.wire,p:v.start}}}} }}\nfunc (it *{collection_name}Iter) Next() ({item_ty},bool) {{ var zero {item_ty};if it.index>=it.view.count{{return zero,false}};value,e:={item_name}(&it.decoder);if e!=nil{{return zero,false}};it.index++;return value,true }}\n"));
    }
    output.push_str(&format!("type {name}LazyView struct{{"));
    for field in &item.fields {
        let ty = if collections
            .iter()
            .any(|candidate| candidate.name == field.name)
        {
            go_lazy_field_name(name, &field.name)
        } else {
            go_lazy_view_type(&field.ty, schema)
        };
        output.push_str(&format!(" {} {};", pascal_case(&field.name), ty));
    }
    output.push_str("}\n");
    output.push_str(&format!("func read{name}LazyView(d *wireDecoder) ({name}LazyView,error){{var v {name}LazyView;if e:=cid(d,{name}CID);e!=nil{{return v,e}};var e error;"));
    for field in &item.fields {
        let lhs = format!("v.{}", pascal_case(&field.name));
        let collection = collections
            .iter()
            .any(|candidate| candidate.name == field.name);
        let decode = if collection {
            let collection_name = go_lazy_field_name(name, &field.name);
            let item_name = go_lazy_item_name(name, &field.name);
            format!(
                "{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};var start=d.p;for i:=0;i<c;i++{{_,e={item_name}(d);if e!=nil{{return v,e}}}};{lhs}={collection_name}{{wire:d.b,start:start,count:c}};}};"
            )
        } else {
            String::new()
        };
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!(
                "if v.{}&(1<<{})!=0{{",
                pascal_case(owner),
                flag_value(item, bit)
            ));
            if collection {
                output.push_str(&decode);
            } else {
                go_decode_lazy_view_type(&field.ty, &lhs, schema, output);
            }
            output.push_str("};");
        } else if collection {
            output.push_str(&decode);
        } else {
            go_decode_lazy_view_type(&field.ty, &lhs, schema, output);
        }
    }
    output.push_str(&format!("return v,e}}\nfunc Borrow{name}Lazy(b []byte) ({name}LazyView,error){{d:=wireDecoder{{b:b}};v,e:=read{name}LazyView(&d);if e==nil{{e=d.done()}};return v,e}}\n\n"));
}

fn generate_go_view_enum(item: &crate::Enum, schema: &Schema, output: &mut String) {
    let name = &item.name;
    output.push_str(&format!("type {name}View interface{{is{name}View()}}\n"));
    for variant in &item.variants {
        let vn = format!("{}{}View", name, variant.name);
        output.push_str(&format!("type {vn} struct{{"));
        for field in &variant.fields {
            output.push_str(&format!(
                " {} {};",
                pascal_case(&field.name),
                go_view_type(&field.ty, schema)
            ));
        }
        output.push_str(&format!("}}\nfunc ({vn})is{name}View(){{}}\n"));
    }
    output.push_str(&format!("func read{name}View(d *wireDecoder) ({name}View,error) {{ c,e:=d.take(8);if e!=nil{{return nil,e}};switch string(c){{"));
    for variant in &item.variants {
        let vn = format!("{}{}View", name, variant.name);
        let cid = variant
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, variant, schema));
        output.push_str(&format!(
            "case string([]byte{{{}}}): var v {vn};",
            cid_bytes(&cid)
        ));
        for field in &variant.fields {
            go_decode_view_type(
                &field.ty,
                &format!("v.{}", pascal_case(&field.name)),
                schema,
                output,
            );
        }
        output.push_str("return v,e;");
    }
    output.push_str("default:return nil,fmt.Errorf(\"unknown constructor\")}}\n");
    output.push_str(&format!("func Borrow{name}(b []byte) ({name}View,error) {{ d:=wireDecoder{{b:b}}; v,e:=read{name}View(&d);if e==nil{{e=d.done()}};return v,e }}\n\n"));
    generate_go_lazy_view_enum(item, schema, output);
}

fn generate_go_lazy_view_enum(item: &crate::Enum, schema: &Schema, output: &mut String) {
    let collections = item
        .variants
        .iter()
        .flat_map(|variant| variant.fields.iter().map(move |field| (variant, field)))
        .filter(|(_, field)| {
            matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty)
        })
        .collect::<Vec<_>>();
    if item.variants.iter().all(|variant| {
        variant
            .fields
            .iter()
            .all(|field| !matches!(field.ty, Type::Primitive(_)))
    }) && collections.is_empty()
    {
        return;
    }
    let name = &item.name;
    for (variant, field) in &collections {
        let collection_name = go_lazy_enum_field_name(name, &variant.name, &field.name);
        let item_name = go_lazy_enum_item_name(name, &variant.name, &field.name);
        let (item_ty, decode_ty) = match &field.ty {
            Type::Vec(ty) => (go_lazy_view_type(ty, schema), ty.as_ref().clone()),
            Type::Map(key, value) => (
                format!(
                    "{}{}{}Entry",
                    name,
                    pascal_case(&variant.name),
                    pascal_case(&field.name)
                ),
                Type::Map(key.clone(), value.clone()),
            ),
            _ => unreachable!(),
        };
        if let Type::Map(key, value) = &field.ty {
            output.push_str(&format!(
                "type {} struct{{ Key {}; Value {} }}\n",
                item_ty,
                go_lazy_view_type(key, schema),
                go_lazy_view_type(value, schema)
            ));
        }
        output.push_str(&format!("type {collection_name} struct{{ wire []byte; start,count int }}\nfunc (v {collection_name}) Len() int {{ return v.count }}\n"));
        output.push_str(&format!(
            "func {item_name}(d *wireDecoder) ({item_ty},error) {{ var v {item_ty}; var e error;"
        ));
        match decode_ty {
            Type::Map(key, value) => {
                go_decode_lazy_view_type(&key, "v.Key", schema, output);
                go_decode_lazy_view_type(&value, "v.Value", schema, output);
            }
            ty => go_decode_lazy_view_type(&ty, "v", schema, output),
        }
        output.push_str("return v,e}\n");
        output.push_str(&format!("func (v {collection_name}) At(i int) ({item_ty},bool) {{ var zero {item_ty}; if i<0||i>=v.count{{return zero,false}}; d:=wireDecoder{{b:v.wire,p:v.start}}; var value {item_ty}; var e error; for n:=0;n<=i;n++{{value,e={item_name}(&d);if e!=nil{{return zero,false}}}}; return value,true }}\n"));
        output.push_str(&format!("type {collection_name}Iter struct{{ view {collection_name}; index int; decoder wireDecoder }}\nfunc (v {collection_name}) Iter() *{collection_name}Iter {{ return &{collection_name}Iter{{view:v,decoder:wireDecoder{{b:v.wire,p:v.start}}}} }}\nfunc (it *{collection_name}Iter) Next() ({item_ty},bool) {{ var zero {item_ty};if it.index>=it.view.count{{return zero,false}};value,e:={item_name}(&it.decoder);if e!=nil{{return zero,false}};it.index++;return value,true }}\n"));
    }
    output.push_str(&format!(
        "type {name}LazyView interface{{is{name}LazyView()}}\n"
    ));
    for variant in &item.variants {
        let variant_name = format!("{}{}LazyView", name, variant.name);
        output.push_str(&format!("type {variant_name} struct{{"));
        for field in &variant.fields {
            let collection = collections
                .iter()
                .any(|(candidate, f)| candidate.name == variant.name && f.name == field.name);
            let ty = if collection {
                go_lazy_enum_field_name(name, &variant.name, &field.name)
            } else {
                go_lazy_view_type(&field.ty, schema)
            };
            output.push_str(&format!(" {} {};", pascal_case(&field.name), ty));
        }
        output.push_str(&format!(
            "}}\nfunc ({variant_name})is{name}LazyView(){{}}\n"
        ));
    }
    output.push_str(&format!("func read{name}LazyView(d *wireDecoder) ({name}LazyView,error){{c,e:=d.take(8);if e!=nil{{return nil,e}};switch string(c){{"));
    for variant in &item.variants {
        let variant_name = format!("{}{}LazyView", name, variant.name);
        let cid = variant
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, variant, schema));
        output.push_str(&format!(
            "case string([]byte{{{}}}):var v {variant_name};",
            cid_bytes(&cid)
        ));
        for field in &variant.fields {
            let collection = collections
                .iter()
                .find(|(candidate, f)| candidate.name == variant.name && f.name == field.name);
            if let Some((_, collection_field)) = collection {
                let collection_name = go_lazy_enum_field_name(name, &variant.name, &field.name);
                let item_name = go_lazy_enum_item_name(name, &variant.name, &field.name);
                if let Type::Map(key, _) = &collection_field.ty {
                    let entry_ty = format!(
                        "{}{}{}Entry",
                        name,
                        pascal_case(&variant.name),
                        pascal_case(&collection_field.name)
                    );
                    let key_check = if matches!(key.as_ref(), Type::Primitive(name) if name == "String")
                        || is_bytes_type(key)
                    {
                        "if i>0&&wireBytesCompare(previousKey,entry.Key)>=0{return nil,fmt.Errorf(\"map keys are not strictly sorted\")};"
                    } else {
                        "if i>0&&previousKey>=entry.Key{return nil,fmt.Errorf(\"map keys are not strictly sorted\")};"
                    };
                    output.push_str(&format!(
                        "{{var n uint64;n,e=d.varint();if e!=nil{{return nil,e}};var c int;c,e=count(n);if e!=nil{{return nil,e}};var start=d.p;var previousKey {};for i:=0;i<c;i++{{var entry {};entry,e={item_name}(d);if e!=nil{{return nil,e}};{}previousKey=entry.Key}};v.{}={collection_name}{{wire:d.b,start:start,count:c}};}};",
                        go_lazy_view_type(key, schema),
                        entry_ty,
                        key_check,
                        pascal_case(&collection_field.name)
                    ));
                } else {
                    output.push_str(&format!(
                        "{{var n uint64;n,e=d.varint();if e!=nil{{return nil,e}};var c int;c,e=count(n);if e!=nil{{return nil,e}};var start=d.p;for i:=0;i<c;i++{{_,e={item_name}(d);if e!=nil{{return nil,e}}}};v.{}={collection_name}{{wire:d.b,start:start,count:c}};}};",
                        pascal_case(&collection_field.name)
                    ));
                }
            } else {
                go_decode_lazy_view_type(
                    &field.ty,
                    &format!("v.{}", pascal_case(&field.name)),
                    schema,
                    output,
                );
            }
        }
        output.push_str("return v,e;");
    }
    output.push_str("default:return nil,fmt.Errorf(\"unknown constructor\")}}\n");
    output.push_str(&format!("func Borrow{name}Lazy(b []byte) ({name}LazyView,error){{d:=wireDecoder{{b:b}};v,e:=read{name}LazyView(&d);if e==nil{{e=d.done()}};return v,e}}\n\n"));
}

fn go_decode_view_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(
            "{lhs},e=d.take({length});if e!=nil{{return v,e}};"
        ));
        return;
    }
    if is_bytes_type(ty) || matches!(ty, Type::Primitive(name) if name == "String") {
        out.push_str(&format!("{lhs},e=d.bytes();if e!=nil{{return v,e}};"));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(&format!(
                "{{tag,e:=d.u8();if e!=nil{{return v,e}};if tag==1{{var x {};",
                go_view_type(item, schema)
            ));
            go_decode_view_type(item, "x", schema, out);
            out.push_str(&format!(
                "{}=&x}}else if tag!=0{{return v,fmt.Errorf(\"invalid optional marker\")}}}};",
                lhs
            ));
        }
        Type::Primitive(name) if schema.items.iter().any(|i| item_name(i) == name) => {
            if go_named_view(name, schema) {
                out.push_str(&format!(
                    "{lhs},e=read{}View(d);if e!=nil{{return v,e}};",
                    name
                ));
            } else {
                out.push_str(&format!(
                    "{lhs},e=decode_{}(d);if e!=nil{{return v,e}};",
                    snake_case(name)
                ));
            }
        }
        Type::Primitive(name) => out.push_str(&format!(
            "{lhs},e=d.{}();if e!=nil{{return v,e}};",
            go_wire_method(name)
        )),
        Type::Vec(item) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make({},c);for i:=range {lhs}{{", go_view_type(ty, schema)));
            go_decode_view_type(item, &format!("{lhs}[i]"), schema, out);
            out.push_str("}};");
        }
        Type::Map(key, value) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make({},c);var previousKey {};for i:=range {lhs}{{", go_view_type(ty, schema), go_view_type(key, schema)));
            go_decode_view_type(key, &format!("{lhs}[i].Key"), schema, out);
            let key_expr = format!("{lhs}[i].Key");
            let key_check = if matches!(key.as_ref(), Type::Primitive(name) if name == "String")
                || is_bytes_type(key)
            {
                format!(
                    "if i>0&&wireBytesCompare(previousKey,{key_expr})>=0{{return v,fmt.Errorf(\"map keys are not strictly sorted\")}};previousKey={key_expr};"
                )
            } else {
                format!(
                    "if i>0&&previousKey>={key_expr}{{return v,fmt.Errorf(\"map keys are not strictly sorted\")}};previousKey={key_expr};"
                )
            };
            out.push_str(&key_check);
            go_decode_view_type(value, &format!("{lhs}[i].Value"), schema, out);
            out.push_str("}};");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn generate_go_struct(item: &crate::Struct, schema: &Schema, output: &mut String) {
    let name = &item.name;
    output.push_str(&format!("type {name} struct {{\n"));
    for field in &item.fields {
        output.push_str(&format!(
            "    {} {}\n",
            pascal_case(&field.name),
            if field.guard.is_some() {
                format!("*{}", go_type(&field.ty, schema))
            } else {
                go_type(&field.ty, schema)
            }
        ));
    }
    output.push_str("}\n");
    let cid = item
        .cid
        .clone()
        .unwrap_or_else(|| constructor_cid_with_schema(item, schema));
    output.push_str(&format!("var {}CID=[]byte{{{}}}\n", name, cid_bytes(&cid)));
    output.push_str(&format!(
        "func encode_{}(e *wireEncoder,v {}){{e.raw({}CID);",
        snake_case(name),
        name,
        name
    ));
    let mut initialized_guard_owners = Vec::new();
    for (owner, owner_type, bit, fields) in go_guard_groups(item, schema) {
        let effective = format!("__typikon_effective_{}", pascal_case(&owner));
        let present = fields
            .iter()
            .map(|field| format!("v.{}!=nil", pascal_case(field)))
            .collect::<Vec<_>>()
            .join("||");
        let declaration = if initialized_guard_owners.contains(&owner) {
            String::new()
        } else {
            initialized_guard_owners.push(owner.clone());
            format!("{}:=v.{};", effective, pascal_case(&owner))
        };
        output.push_str(&format!(
            "{}if {}{{{}|={}(1<<{})}}else{{{}&^={}(1<<{})}};",
            declaration, present, effective, owner_type, bit, effective, owner_type, bit
        ));
    }
    for field in &item.fields {
        let expr = format!("v.{}", pascal_case(&field.name));
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            let effective = format!("__typikon_effective_{}", pascal_case(owner));
            output.push_str(&format!(
                "if {}&(1<<{})!=0{{",
                effective,
                flag_value(item, bit)
            ));
            if let Some(length) = field.exact_len {
                output.push_str(&format!(
                    "if len(*{})!={length}{{panic(\"invalid exact byte length\")}};",
                    expr
                ));
            }
            go_encode_go_type(&field.ty, &format!("*{}", expr), schema, output);
            output.push_str("};");
        } else {
            let effective = go_guard_groups(item, schema)
                .iter()
                .any(|(owner, _, _, _)| owner == &field.name);
            let encode_expr = if effective {
                format!("__typikon_effective_{}", pascal_case(&field.name))
            } else {
                expr
            };
            if let Some(length) = field.exact_len {
                output.push_str(&format!(
                    "if len({encode_expr})!={length}{{panic(\"invalid exact byte length\")}};"
                ));
            }
            go_encode_go_type(&field.ty, &encode_expr, schema, output);
        }
    }
    output.push_str("}\n");
    output.push_str(&format!("func decode_{}(d *wireDecoder)({name},error){{var v {name};if e:=cid(d,{}CID);e!=nil{{return v,e}};var e error;",snake_case(name),name));
    for field in &item.fields {
        let lhs = format!("v.{}", pascal_case(&field.name));
        if let Some(guard) = &field.guard {
            let (_, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!("if v.Flags&(1<<{})!=0{{", flag_value(item, bit)));
            let temporary = format!("guarded_{}", field.name);
            output.push_str(&format!("var {temporary} {};", go_type(&field.ty, schema)));
            go_decode_go_type(&field.ty, &temporary, schema, output, true);
            if let Some(length) = field.exact_len {
                output.push_str(&format!("if len({temporary})!={length}{{return v,fmt.Errorf(\"invalid exact byte length\")}};"));
            }
            output.push_str(&format!("v.{}=&{};", pascal_case(&field.name), temporary));
            output.push_str("};");
        } else {
            go_decode_go_type(&field.ty, &lhs, schema, output, false);
            if let Some(length) = field.exact_len {
                output.push_str(&format!(
                    "if len({lhs})!={length}{{return v,fmt.Errorf(\"invalid exact byte length\")}};"
                ));
            }
        }
    }
    output.push_str("return v,e}\n");
}

fn go_guard_groups(
    item: &crate::Struct,
    schema: &Schema,
) -> Vec<(String, String, u64, Vec<String>)> {
    let mut groups: Vec<(String, String, u64, Vec<String>)> = Vec::new();
    for field in &item.fields {
        let Some(guard) = &field.guard else { continue };
        let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
        let bit_value = flag_value(item, bit);
        let owner_type = item
            .fields
            .iter()
            .find(|candidate| candidate.name == owner)
            .map(|candidate| go_type(&candidate.ty, schema))
            .unwrap_or_else(|| owner.to_owned());
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.0 == owner && group.2 == bit_value)
        {
            group.3.push(field.name.clone());
        } else {
            groups.push((
                owner.to_owned(),
                owner_type,
                bit_value,
                vec![field.name.clone()],
            ));
        }
    }
    groups
}

fn generate_go_enum(item: &crate::Enum, schema: &Schema, output: &mut String) {
    let name = &item.name;
    output.push_str(&format!("type {name} interface{{is{name}()}}\n"));
    for v in &item.variants {
        let vn = format!("{}{}", name, v.name);
        output.push_str(&format!("type {vn} struct{{"));
        for f in &v.fields {
            output.push_str(&format!(
                "{} {};",
                pascal_case(&f.name),
                go_type(&f.ty, schema)
            ));
        }
        output.push_str(&format!("}}\nfunc ({vn})is{name}(){{}}\n"));
    }
    output.push_str(&format!(
        "func encode_{}(e *wireEncoder,v {name}){{switch x:=v.(type){{",
        snake_case(name)
    ));
    for v in &item.variants {
        let vn = format!("{}{}", name, v.name);
        let cid = v
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, v, schema));
        output.push_str(&format!("case {vn}:e.raw([]byte{{{}}});", cid_bytes(&cid)));
        for f in &v.fields {
            if let Some(length) = f.exact_len {
                output.push_str(&format!(
                    "if len(x.{})!={length}{{panic(\"invalid exact byte length\")}};",
                    pascal_case(&f.name)
                ));
            }
            go_encode_go_type(
                &f.ty,
                &format!("x.{}", pascal_case(&f.name)),
                schema,
                output,
            );
        }
    }
    output.push_str("default:panic(\"unknown variant\")}}\n");
    output.push_str(&format!("func decode_{}(d *wireDecoder)({name},error){{var v {name};c,e:=d.take(8);if e!=nil{{return nil,e}};switch string(c){{",snake_case(name)));
    for v in &item.variants {
        let vn = format!("{}{}", name, v.name);
        let cid = v
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, v, schema));
        output.push_str(&format!(
            "case string([]byte{{{}}}):var x {vn};",
            cid_bytes(&cid)
        ));
        for f in &v.fields {
            let expr = format!("x.{}", pascal_case(&f.name));
            go_decode_go_type(&f.ty, &expr, schema, output, false);
            if let Some(length) = f.exact_len {
                output.push_str(&format!("if len({expr})!={length}{{return nil,fmt.Errorf(\"invalid exact byte length\")}};"));
            }
        }
        output.push_str("return x,e;");
    }
    output.push_str("default:return nil,fmt.Errorf(\"unknown constructor\")}}\n");
}

fn cid_bytes(cid: &str) -> String {
    cid.as_bytes()
        .chunks(2)
        .map(|x| format!("0x{}", String::from_utf8_lossy(x)))
        .collect::<Vec<_>>()
        .join(",")
}
fn flag_value(item: &crate::Struct, bit: &str) -> u64 {
    item.fields
        .iter()
        .find(|f| f.name == "flags")
        .map(|_| match bit {
            "is_bot" => 0,
            "is_verified" => 1,
            "has_avatar" => 2,
            _ => 0,
        })
        .unwrap_or(0)
}
fn go_wire_method(name: &str) -> &str {
    match name {
        "String" => "string",
        "bool" => "bool",
        "f32" => "f32",
        "f64" => "f64",
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" => "u64",
        "i8" => "i8",
        "i16" => "i16",
        "i32" => "i32",
        "i64" => "i64",
        _ => "bytes",
    }
}
fn go_encode_go_type(ty: &Type, expr: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(
            "if len({expr})!={length}{{panic(\"invalid fixed bytes length\")}};e.raw({expr}[:]);"
        ));
        return;
    }
    if is_bytes_type(ty) {
        out.push_str(&format!("e.bytes({expr});"));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(&format!("if {expr}==nil{{e.u8(0)}}else{{e.u8(1);"));
            go_encode_go_type(item, &format!("*({expr})"), schema, out);
            out.push_str("};");
        }
        Type::Primitive(n) => {
            if let Some(Item::Alias(alias)) = schema.items.iter().find(|i| item_name(i) == n) {
                if let Some(length) = alias.exact_len {
                    out.push_str(&format!(
                        "if len({expr})!={length}{{panic(\"invalid exact length\")}};"
                    ));
                }
                go_encode_go_type(&alias.ty, expr, schema, out);
            } else if schema.items.iter().any(|i| item_name(i) == n) {
                out.push_str(&format!("encode_{}(e,{expr});", snake_case(n)))
            } else {
                out.push_str(&format!("e.{}({expr});", go_wire_method(n)))
            }
        }
        Type::Vec(t) => {
            let x = "item";
            out.push_str(&format!(
                "e.varint(uint64(len({expr})));for _,{x}:=range {expr}{{"
            ));
            go_encode_go_type(t, x, schema, out);
            out.push_str("};")
        }
        Type::Map(_, v) => {
            out.push_str(&format!("keys:=make([]string,0,len({expr}));for k:=range {expr}{{keys=append(keys,k)}};sort.Strings(keys);e.varint(uint64(len(keys)));for _,k:=range keys{{e.string(k);"));
            go_encode_go_type(v, &format!("{expr}[k]"), schema, out);
            out.push_str("};")
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}
fn go_decode_go_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String, guarded: bool) {
    let _ = guarded;
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(
            "tmp,e:=d.take({length});if e!=nil{{return v,e}};copy({lhs}[:],tmp);"
        ));
        return;
    }
    if is_bytes_type(ty) {
        out.push_str(&format!("{lhs},e=d.bytes();if e!=nil{{return v,e}};"));
        return;
    }
    match ty {
        Type::Optional(item) => {
            let tag = format!(
                "optionalTag{}",
                lhs.chars()
                    .filter(|c| c.is_ascii_alphanumeric())
                    .collect::<String>()
            );
            out.push_str(&format!(
                "{tag},e:=d.u8();if e!=nil{{return v,e}};if {tag}==1{{var x {};",
                go_type(item, schema)
            ));
            go_decode_go_type(item, "x", schema, out, false);
            out.push_str(&format!(
                "{}=&x}}else if {tag}!=0{{return v,fmt.Errorf(\"invalid optional marker\")}};",
                lhs
            ));
        }
        Type::Primitive(n) => {
            if let Some(Item::Alias(alias)) = schema.items.iter().find(|i| item_name(i) == n) {
                go_decode_go_type(&alias.ty, lhs, schema, out, guarded);
                if let Some(length) = alias.exact_len {
                    out.push_str(&format!(
                        "if len({lhs})!={length}{{return v,fmt.Errorf(\"invalid exact length\")}};"
                    ));
                }
            } else if schema.items.iter().any(|i| item_name(i) == n) {
                out.push_str(&format!(
                    "{lhs},e=decode_{}(d);if e!=nil{{return v,e}};",
                    snake_case(n)
                ))
            } else {
                out.push_str(&format!(
                    "{lhs},e=d.{}();if e!=nil{{return v,e}};",
                    go_wire_method(n)
                ))
            }
        }
        Type::Vec(t) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make([]{},c);for i:=range {lhs}{{",go_type(t,schema)));
            go_decode_go_type(t, &format!("{lhs}[i]"), schema, out, false);
            out.push_str("};};")
        }
        Type::Map(_, v) => {
            out.push_str(&format!("{{var n uint64;n,e=d.varint();if e!=nil{{return v,e}};var c int;c,e=count(n);if e!=nil{{return v,e}};{lhs}=make(map[string]{},c);for i:=0;i<c;i++{{k,e:=d.string();if e!=nil{{return v,e}};",go_type(v,schema)));
            go_decode_go_type(v, &format!("{lhs}[k]"), schema, out, false);
            out.push_str("};};")
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

pub fn generate_typescript_binding(schema: &Schema) -> String {
    let mut output = String::from(
        "export interface TypikonNative { encodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; decodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; validateBinary(layer: number, typeName: string, input: Uint8Array): void; }\n\nclass WireEncoder { private b: number[] = []; raw(v: Uint8Array): void { for (const x of v) this.b.push(x); } u8(v: number): void { this.b.push(v & 255); } u16(v: number): void { this.u8(v); this.u8(v >>> 8); } u32(v: number): void { this.u8(v); this.u8(v >>> 8); this.u8(v >>> 16); this.u8(v >>> 24); } u64(v: bigint): void { let n = BigInt.asUintN(64, v); for (let i = 0n; i < 8n; i++) { this.u8(Number(n & 255n)); n >>= 8n; } } i8(v: number): void { this.u8(v); } i16(v: number): void { this.u16(v); } i32(v: number): void { this.u32(v); } i64(v: bigint): void { this.u64(v); } f32(v: number): void { const x = new DataView(new ArrayBuffer(4)); x.setFloat32(0, v, true); this.u32(x.getUint32(0, true)); } f64(v: number): void { const x = new DataView(new ArrayBuffer(8)); x.setFloat64(0, v, true); this.u64(x.getBigUint64(0, true)); } bool(v: boolean): void { this.u8(v ? 1 : 0); } varint(v: number): void { let n = BigInt(v); while (n >= 128n) { this.u8(Number(n & 127n) | 128); n >>= 7n; } this.u8(Number(n)); } bytes(v: Uint8Array): void { this.varint(v.length); this.raw(v); } string(v: string): void { this.bytes(new TextEncoder().encode(v)); } finish(): Uint8Array { if (this.b.length > 4 * 1024 * 1024) throw new Error('packet exceeds limit'); return Uint8Array.from(this.b); } }\nclass WireDecoder { private p = 0; constructor(private readonly b: Uint8Array) {} take(n: number): Uint8Array { if (n < 0 || this.p > this.b.length - n) throw new Error('truncated wire'); const v = this.b.subarray(this.p, this.p + n); this.p += n; return v; } u8(): number { return this.take(1)[0]; } u16(): number { return this.u8() | (this.u8() << 8); } u32(): number { return (this.u8() | (this.u8() << 8) | (this.u8() << 16) | (this.u8() << 24)) >>> 0; } u64(): bigint { let n = 0n; for (let i = 0n; i < 8n; i++) n |= BigInt(this.u8()) << (8n * i); return n; } i8(): number { return (this.u8() << 24) >> 24; } i16(): number { const n = this.u16(); return (n << 16) >> 16; } i32(): number { return this.u32() | 0; } i64(): bigint { return BigInt.asIntN(64, this.u64()); } f32(): number { const x = new DataView(this.take(4).slice().buffer); return x.getFloat32(0, true); } f64(): number { const x = new DataView(this.take(8).slice().buffer); return x.getFloat64(0, true); } bool(): boolean { return this.u8() !== 0; } varint(): number { let n = 0n; for (let i = 0n; i < 10n; i++) { const b = this.u8(); n |= BigInt(b & 127) << (7n * i); if (b < 128) return Number(n); } throw new Error('varint overflow'); } bytes(): Uint8Array { const n = this.varint(); return this.take(n); } string(): string { return new TextDecoder().decode(this.bytes()); } done(): void { if (this.p !== this.b.length) throw new Error('trailing bytes'); } }\nconst cid = (d: WireDecoder, want: Uint8Array): void => { const got = d.take(8); for (let i = 0; i < 8; i++) if (got[i] !== want[i]) throw new Error('invalid constructor ID'); };\nconst hex = (s: string): Uint8Array => Uint8Array.from(s.match(/.{2}/g)!.map(x => parseInt(x, 16)));\n\n",
    );
    output = output
        .replace("class WireDecoder {", "export class WireDecoder {")
        .replace("constructor(private readonly b: Uint8Array) {} take(n: number): Uint8Array", "constructor(private readonly b: Uint8Array, p = 0) { this.p = p; } position(): number { return this.p; } take(n: number): Uint8Array")
        .replace("done(): void { if (this.p !== this.b.length)", "seek(position: number): void { if (position < 0 || position > this.b.length) throw new Error('invalid decoder position'); this.p = position; } done(): void { if (this.p !== this.b.length)");
    output.push_str("export class LazyCollection<T> { constructor(private readonly wire: Uint8Array, private readonly start: number, readonly length: number, private readonly decode: (decoder: WireDecoder) => T) {} at(index: number): T { if (!Number.isInteger(index) || index < 0 || index >= this.length) throw new RangeError('collection index out of range'); const decoder = new WireDecoder(this.wire, this.start); let value!: T; for (let i = 0; i <= index; i++) value = this.decode(decoder); return value; } *[Symbol.iterator](): IterableIterator<T> { const decoder = new WireDecoder(this.wire, this.start); for (let i = 0; i < this.length; i++) yield this.decode(decoder); } }\nexport class BorrowedPacket<T> { constructor(readonly wire: Uint8Array, readonly view: T) {} }\n\n");
    output.push_str("const wireBytesCompare = (a: Uint8Array, b: Uint8Array): number => { for (let i = 0; i < Math.min(a.length, b.length); i++) { if (a[i] < b[i]) return -1; if (a[i] > b[i]) return 1; } return a.length - b.length; };\n\n");
    output = output.replace(
        "validateBinary(layer: number, typeName: string, input: Uint8Array): void;",
        "validateBinary(layer: number, typeName: string, input: Uint8Array): void; borrowBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array;",
    );
    for item in &schema.items {
        let name = item_name(item);
        if let Item::Struct(item) = item {
            output.push_str(&format!("export interface {name} {{\n"));
            for field in &item.fields {
                let optional = if field.guard.is_some() { "?" } else { "" };
                output.push_str(&format!(
                    "  {}{}: {};\n",
                    field.name,
                    optional,
                    typescript_type(&field.ty, schema)
                ));
            }
            output.push_str("}\n\n");
        } else {
            output.push_str(&format!("{}\n\n", typescript_item_type(item, schema)));
        }
        let function_name = name.to_ascii_lowercase();
        output.push_str(&format!("export function encodeBinary{name}(native: TypikonNative, wire: Uint8Array): Uint8Array {{ return native.encodeBinary({}, \"{}\", wire); }}\nexport function decodeBinary{name}(native: TypikonNative, wire: Uint8Array): Uint8Array {{ return native.decodeBinary({}, \"{}\", wire); }}\nexport function validateBinary{name}(native: TypikonNative, wire: Uint8Array): void {{ native.validateBinary({}, \"{}\", wire); }}\n\n", schema.version, function_name, schema.version, function_name, schema.version, function_name));
        generate_typescript_typed_item(item, schema, &mut output);
    }
    output
}

fn generate_typescript_typed_item(item: &Item, schema: &Schema, output: &mut String) {
    match item {
        Item::Alias(alias) => {
            let name = &alias.name;
            let lower = name.to_ascii_lowercase();
            let mut encode = String::new();
            if let Some(length) = alias.exact_len {
                encode.push_str(&format!(
                    " if (value.length !== {length}) throw new Error('invalid exact length');"
                ));
            }
            typescript_encode_type(&alias.ty, "value", schema, &mut encode);
            let mut decode = String::new();
            typescript_decode_expression(&alias.ty, schema, &mut decode);
            if let Some(length) = alias.exact_len {
                decode = format!(
                    "(()=>{{const value = {decode}; if (value.length !== {length}) throw new Error('invalid exact length'); return value;}})()",
                    decode = decode,
                    length = length
                );
            }
            output.push_str(&format!(
                "function write_{lower}(e: WireEncoder, value: {name}): void {{{encode}}}\nfunction read_{lower}(d: WireDecoder): {name} {{ return {decode}; }}\nexport function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{lower}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{lower}(d); d.done(); return value; }}\n\n"
            ));
        }
        Item::Struct(st) => {
            let name = &st.name;
            let fn_name = name.to_ascii_lowercase();
            let cid = st
                .cid
                .clone()
                .unwrap_or_else(|| constructor_cid_with_schema(st, schema));
            output.push_str(&format!("const {name}CID = hex(\"{cid}\");\nfunction write_{fn_name}(e: WireEncoder, value: {name}): void {{ e.raw({name}CID);"));
            let mut initialized_guard_owners = Vec::new();
            for (owner, bit, fields) in ts_guard_groups(st, schema) {
                let present = fields
                    .iter()
                    .map(|field| format!("value.{} !== undefined", field))
                    .collect::<Vec<_>>()
                    .join(" || ");
                let declaration = if initialized_guard_owners.contains(&owner) {
                    String::new()
                } else {
                    initialized_guard_owners.push(owner.clone());
                    format!(" let effective_{owner} = value.{owner};")
                };
                output.push_str(&format!("{declaration} if ({present}) effective_{owner} |= (1 << {bit}); else effective_{owner} &= ~(1 << {bit});"));
            }
            for field in &st.fields {
                let expr = format!("value.{}", field.name);
                if let Some(guard) = &field.guard {
                    let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
                    output.push_str(&format!(
                        " if ((effective_{} & (1 << {})) !== 0) {{",
                        owner,
                        ts_guard_bit(schema, owner, bit)
                    ));
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!(
                            " if ({}!.length !== {}) throw new Error('invalid exact byte length');",
                            expr, length
                        ));
                    }
                    typescript_encode_type(&field.ty, &format!("{}!", expr), schema, output);
                    output.push_str(" }");
                } else {
                    let encode_expr = if ts_guard_groups(st, schema)
                        .iter()
                        .any(|(owner, _, _)| owner == &field.name)
                    {
                        format!("effective_{}", field.name)
                    } else {
                        expr
                    };
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!(
                            " if ({}.length !== {}) throw new Error('invalid exact byte length');",
                            encode_expr, length
                        ));
                    }
                    typescript_encode_type(&field.ty, &encode_expr, schema, output);
                }
            }
            output.push_str(" }\n");
            output.push_str(&format!("function read_{fn_name}(d: WireDecoder): {name} {{ cid(d, {name}CID); const value = {{}} as {name};"));
            for field in &st.fields {
                let lhs = format!("value.{}", field.name);
                if let Some(guard) = &field.guard {
                    let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
                    output.push_str(&format!(
                        " if ((value.{} & (1 << {})) !== 0) {{",
                        owner,
                        ts_guard_bit(schema, owner, bit)
                    ));
                    typescript_decode_type(&field.ty, &lhs, schema, output);
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!(
                            " if ({}.length !== {}) throw new Error('invalid exact byte length');",
                            lhs, length
                        ));
                    }
                    output.push_str(" }");
                } else {
                    typescript_decode_type(&field.ty, &lhs, schema, output);
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!(
                            " if ({}.length !== {}) throw new Error('invalid exact byte length');",
                            lhs, length
                        ));
                    }
                }
            }
            output.push_str(" return value; }\n");
            output.push_str(&format!("export function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{fn_name}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{fn_name}(d); d.done(); return value; }}\n\n"));
            generate_typescript_view_struct(st, schema, output);
        }
        Item::Flags(flags) => {
            let name = &flags.name;
            let method = typescript_wire_method(&flags.underlying);
            let lower = name.to_ascii_lowercase();
            output.push_str(&format!("function write_{lower}(e: WireEncoder, value: {name}): void {{ e.{method}(value); }}\nfunction read_{lower}(d: WireDecoder): {name} {{ return d.{method}(); }}\nexport function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{lower}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{lower}(d); d.done(); return value; }}\n\n"));
        }
        Item::Enum(en) if en.variants.iter().all(|v| v.fields.is_empty()) => {
            let name = &en.name;
            output.push_str(&format!(
                "function write_{}(e: WireEncoder, value: {}): void {{ switch (value) {{",
                name.to_ascii_lowercase(),
                name
            ));
            for v in &en.variants {
                output.push_str(&format!(
                    " case \"{}\": e.u64({}n); break;",
                    v.name,
                    v.value.unwrap_or_default()
                ));
            }
            output.push_str(" } }\n");
            output.push_str(&format!(
                "function read_{}(d: WireDecoder): {} {{ switch (d.u64()) {{",
                name.to_ascii_lowercase(),
                name
            ));
            for v in &en.variants {
                output.push_str(&format!(
                    " case {}n: return \"{}\";",
                    v.value.unwrap_or_default(),
                    v.name
                ));
            }
            output.push_str(" default: throw new Error('invalid enum'); } }\n");
            output.push_str(&format!("export function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{}(d); d.done(); return value; }}\n\n", name.to_ascii_lowercase(), name.to_ascii_lowercase()));
        }
        Item::Enum(en) => {
            let name = &en.name;
            let lower = name.to_ascii_lowercase();
            output.push_str(&format!(
                "function write_{lower}(e: WireEncoder, value: {name}): void {{"
            ));
            for variant in &en.variants {
                let cid = variant
                    .cid
                    .clone()
                    .unwrap_or_else(|| variant_cid_with_schema(en, variant, schema));
                output.push_str(&format!(
                    " if (\"{}\" in value) {{ e.raw(hex(\"{}\"));",
                    variant.name, cid
                ));
                for field in &variant.fields {
                    let expr = format!("value.{}.{}", variant.name, field.name);
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!(
                            " if ({}.length !== {}) throw new Error('invalid exact byte length');",
                            expr, length
                        ));
                    }
                    typescript_encode_type(&field.ty, &expr, schema, output);
                }
                output.push_str(" return; }");
            }
            output.push_str(" throw new Error('unknown variant'); }\n");
            output.push_str(&format!(
                "function read_{lower}(d: WireDecoder): {name} {{ const c = d.take(8);"
            ));
            for variant in &en.variants {
                let cid = variant
                    .cid
                    .clone()
                    .unwrap_or_else(|| variant_cid_with_schema(en, variant, schema));
                output.push_str(&format!(
                    " if (c.every((x, i) => x === hex(\"{}\")[i])) {{ return {{ {}: {{",
                    cid, variant.name
                ));
                for field in &variant.fields {
                    output.push_str(&format!(" {}: ", field.name));
                    let mut decoded = String::new();
                    typescript_decode_expression(&field.ty, schema, &mut decoded);
                    if let Some(length) = field.exact_len {
                        output.push_str(&format!("(() => {{ const value = {decoded}; if (value.length !== {length}) throw new Error('invalid exact byte length'); return value; }})()"));
                    } else {
                        output.push_str(&decoded);
                    }
                    output.push(',');
                }
                output.push_str(" } }; }");
            }
            output.push_str(" throw new Error('unknown constructor'); }\n");
            output.push_str(&format!("export function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{lower}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{lower}(d); d.done(); return value; }}\n\n"));
            generate_typescript_view_enum(en, schema, output);
        }
    }
}

fn typescript_wire_method(name: &str) -> &str {
    match name {
        "String" => "string",
        "bool" => "bool",
        "f32" => "f32",
        "f64" => "f64",
        "u8" => "u8",
        "u16" => "u16",
        "u32" => "u32",
        "u64" => "u64",
        "i8" => "i8",
        "i16" => "i16",
        "i32" => "i32",
        "i64" => "i64",
        _ => "u8",
    }
}
fn ts_guard_bit(schema: &Schema, owner: &str, bit: &str) -> u64 {
    schema
        .items
        .iter()
        .find_map(|item| {
            if let Item::Flags(flags) = item {
                if item_name(item) == owner || owner == "flags" {
                    flags.bits.iter().find(|x| x.name == bit).map(|x| x.value)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or(match bit {
            "is_bot" => 0,
            "is_verified" => 1,
            "has_avatar" => 2,
            _ => 0,
        })
}
fn typescript_encode_type(ty: &Type, expr: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(
            " if ({}.length !== {}) throw new Error('invalid fixed bytes length'); e.raw({});",
            expr, length, expr
        ));
        return;
    }
    if is_bytes_type(ty) {
        out.push_str(&format!(" e.bytes({});", expr));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(&format!(
                " e.u8({} === undefined ? 0 : 1); if ({} !== undefined) {{",
                expr, expr
            ));
            typescript_encode_type(item, expr, schema, out);
            out.push_str(" }");
        }
        Type::Primitive(n) => {
            if let Some(Item::Alias(alias)) = schema.items.iter().find(|i| item_name(i) == n) {
                if let Some(length) = alias.exact_len {
                    out.push_str(&format!(
                        " if ({}.length !== {}) throw new Error('invalid exact length');",
                        expr, length
                    ));
                }
                typescript_encode_type(&alias.ty, expr, schema, out);
            } else if schema.items.iter().any(|i| item_name(i) == n) {
                out.push_str(&format!(" write_{}(e, {});", n.to_ascii_lowercase(), expr));
            } else {
                out.push_str(&format!(" e.{}({});", typescript_wire_method(n), expr));
            }
        }
        Type::Vec(item) => {
            out.push_str(&format!(
                " e.varint({}.length); for (const item of {}) {{",
                expr, expr
            ));
            typescript_encode_type(item, "item", schema, out);
            out.push_str(" }");
        }
        Type::Map(_, value) => {
            out.push_str(&format!(" const keys = Object.keys({}).sort(); e.varint(keys.length); for (const key of keys) {{ e.string(key);", expr));
            typescript_encode_type(value, &format!("{}[key]", expr), schema, out);
            out.push_str(" }");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}
fn typescript_decode_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(" {} = d.take({});", lhs, length));
        return;
    }
    if is_bytes_type(ty) {
        out.push_str(&format!(" {} = d.bytes();", lhs));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(" { const tag = d.u8(); if (tag === 1) {");
            typescript_decode_type(item, lhs, schema, out);
            out.push_str(&format!(" }} else if (tag === 0) {{ {} = undefined; }} else throw new Error('invalid optional marker'); }}", lhs));
        }
        Type::Primitive(n) => {
            if let Some(Item::Alias(alias)) = schema.items.iter().find(|i| item_name(i) == n) {
                typescript_decode_type(&alias.ty, lhs, schema, out);
                if let Some(length) = alias.exact_len {
                    out.push_str(&format!(
                        " if ({}.length !== {}) throw new Error('invalid exact length');",
                        lhs, length
                    ));
                }
            } else if schema.items.iter().any(|i| item_name(i) == n) {
                out.push_str(&format!(" {} = read_{}(d);", lhs, n.to_ascii_lowercase()));
            } else {
                out.push_str(&format!(" {} = d.{}();", lhs, typescript_wire_method(n)));
            }
        }
        Type::Vec(item) => {
            out.push_str(&format!(
                " {lhs} = []; for (let i = d.varint(); i > 0; i--) {{",
                lhs = lhs
            ));
            typescript_decode_type(item, &format!("{}[{}.length]", lhs, lhs), schema, out);
            out.push_str(" }");
        }
        Type::Map(_, value) => {
            out.push_str(&format!(
                " {} = {{}}; for (let i = d.varint(); i > 0; i--) {{ const key = d.string();",
                lhs
            ));
            typescript_decode_type(value, &format!("{}[key]", lhs), schema, out);
            out.push_str(" }");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}
fn typescript_decode_expression(ty: &Type, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!("d.take({length})"));
        return;
    }
    if is_bytes_type(ty) {
        out.push_str("d.bytes()");
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str("(()=>{const tag=d.u8();if(tag===0)return undefined;if(tag!==1)throw new Error('invalid optional marker');return ");
            typescript_decode_expression(item, schema, out);
            out.push_str(";})()");
        }
        Type::Primitive(n) => {
            if schema.items.iter().any(|i| item_name(i) == n) {
                out.push_str(&format!("read_{}(d)", n.to_ascii_lowercase()));
            } else {
                out.push_str(&format!("d.{}()", typescript_wire_method(n)));
            }
        }
        Type::Vec(item) => {
            out.push_str("Array.from({ length: d.varint() }, () => ");
            typescript_decode_expression(item, schema, out);
            out.push(')');
        }
        Type::Map(_, value) => {
            out.push_str("Object.fromEntries(Array.from({ length: d.varint() }, () => { const key = d.string(); return [key, ");
            typescript_decode_expression(value, schema, out);
            out.push_str("]; }))");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn typescript_view_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Optional(item) => format!("{} | undefined", typescript_view_type(item, schema)),
        Type::FixedBytes(_) => "Uint8Array".into(),
        Type::Primitive(name) if name == "String" => "Uint8Array".into(),
        Type::Primitive(name) if schema.items.iter().any(|i| item_name(i) == name) => {
            if typescript_named_view(name, schema) {
                format!("{}View", name)
            } else {
                name.clone()
            }
        }
        Type::Vec(_) if is_bytes_type(ty) => "Uint8Array".into(),
        Type::Vec(item) => format!("Array<{}>", typescript_view_type(item, schema)),
        Type::Map(key, value) => format!(
            "Array<{{ key: {}; value: {} }}>",
            typescript_view_type(key, schema),
            typescript_view_type(value, schema)
        ),
        Type::Primitive(name) => typescript_type(&Type::Primitive(name.clone()), schema),
    }
}

fn ts_guard_groups(item: &crate::Struct, schema: &Schema) -> Vec<(String, u64, Vec<String>)> {
    let mut groups: Vec<(String, u64, Vec<String>)> = Vec::new();
    for field in &item.fields {
        let Some(guard) = &field.guard else { continue };
        let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
        let bit_value = ts_guard_bit(schema, owner, bit);
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.0 == owner && group.1 == bit_value)
        {
            group.2.push(field.name.clone());
        } else {
            groups.push((owner.to_owned(), bit_value, vec![field.name.clone()]));
        }
    }
    groups
}

fn generate_typescript_view_struct(item: &crate::Struct, schema: &Schema, output: &mut String) {
    let name = &item.name;
    let lower = name.to_ascii_lowercase();
    output.push_str(&format!("export interface {name}View {{"));
    for field in &item.fields {
        output.push_str(&format!(
            " {}{}: {};",
            field.name,
            if field.guard.is_some() { "?" } else { "" },
            typescript_view_type(&field.ty, schema)
        ));
    }
    output.push_str(" }\n");
    output.push_str(&format!("function read_{lower}_view(d: WireDecoder): {name}View {{ cid(d, {name}CID); const value = {{}} as {name}View;"));
    for field in &item.fields {
        let lhs = format!("value.{}", field.name);
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!(
                " if ((value.{} & (1 << {})) !== 0) {{",
                owner,
                ts_guard_bit(schema, owner, bit)
            ));
            typescript_decode_view_type(&field.ty, &lhs, schema, output);
            output.push_str(" }");
        } else {
            typescript_decode_view_type(&field.ty, &lhs, schema, output);
        }
    }
    output.push_str(" return value; }\n");
    output.push_str(&format!("export function decode{name}View(wire: Uint8Array): {name}View {{ const d = new WireDecoder(wire); const value = read_{lower}_view(d); d.done(); return value; }}\nexport function borrow{name}View(wire: Uint8Array): BorrowedPacket<{name}View> {{ return new BorrowedPacket(wire, decode{name}View(wire)); }}\n\n"));
    generate_typescript_lazy_view_struct(item, schema, output);
}

fn typescript_lazy_item_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Map(key, value) => format!(
            "{{ key: {}; value: {} }}",
            typescript_lazy_view_type(key, schema),
            typescript_lazy_view_type(value, schema)
        ),
        Type::Vec(item) => typescript_lazy_view_type(item, schema),
        _ => typescript_view_type(ty, schema),
    }
}

fn typescript_has_lazy_view(name: &str, schema: &Schema) -> bool {
    schema
        .items
        .iter()
        .find_map(|item| match item {
            Item::Struct(st) if st.name == name => Some(st.fields.iter().any(|field| {
                matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty)
            })),
            _ => None,
        })
        .unwrap_or(false)
}

fn typescript_lazy_view_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Vec(_) if is_bytes_type(ty) => "Uint8Array".into(),
        Type::Primitive(name)
            if schema.items.iter().any(|item| item_name(item) == name)
                && typescript_has_lazy_view(name, schema) =>
        {
            format!("{}LazyView", name)
        }
        Type::Vec(item) => format!("Array<{}>", typescript_lazy_view_type(item, schema)),
        Type::Map(key, value) => format!(
            "Array<{{ key: {}; value: {} }}>",
            typescript_lazy_view_type(key, schema),
            typescript_lazy_view_type(value, schema)
        ),
        _ => typescript_view_type(ty, schema),
    }
}

fn typescript_decode_lazy_expression(ty: &Type, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!("d.take({length})"));
        return;
    }
    if is_bytes_type(ty) || matches!(ty, Type::Primitive(name) if name == "String") {
        out.push_str("d.bytes()");
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str("(()=>{const tag=d.u8();if(tag===0)return undefined;if(tag!==1)throw new Error('invalid optional marker');return ");
            typescript_decode_lazy_expression(item, schema, out);
            out.push_str(";})()");
        }
        Type::Primitive(name) if schema.items.iter().any(|item| item_name(item) == name) => {
            let suffix = if typescript_has_lazy_view(name, schema) {
                "_lazy_view"
            } else if typescript_named_view(name, schema) {
                "_view"
            } else {
                ""
            };
            if suffix == "_lazy_view" {
                out.push_str(&format!(
                    "read_{}_lazy_view(d, wire)",
                    name.to_ascii_lowercase()
                ));
            } else {
                out.push_str(&format!("read_{}{}(d)", name.to_ascii_lowercase(), suffix));
            }
        }
        Type::Primitive(name) => out.push_str(&format!("d.{}()", typescript_wire_method(name))),
        Type::Vec(item) => {
            out.push_str("Array.from({ length: d.varint() }, () => ");
            typescript_decode_lazy_expression(item, schema, out);
            out.push(')');
        }
        Type::Map(key, value) => {
            out.push_str("Array.from({ length: d.varint() }, () => ({ key: ");
            typescript_decode_lazy_expression(key, schema, out);
            out.push_str(", value: ");
            typescript_decode_lazy_expression(value, schema, out);
            out.push_str(" }))");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn typescript_decode_lazy_field_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    if let Type::Primitive(name) = ty
        && schema.items.iter().any(|item| item_name(item) == name)
        && typescript_has_lazy_view(name, schema)
    {
        out.push_str(&format!(
            " {} = read_{}_lazy_view(d, wire);",
            lhs,
            name.to_ascii_lowercase()
        ));
        return;
    }
    typescript_decode_view_type(ty, lhs, schema, out);
}

fn generate_typescript_lazy_view_struct(
    item: &crate::Struct,
    schema: &Schema,
    output: &mut String,
) {
    let collections = item
        .fields
        .iter()
        .filter(|field| {
            matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty)
        })
        .collect::<Vec<_>>();
    if collections.is_empty() {
        return;
    }
    let name = &item.name;
    let lower = name.to_ascii_lowercase();
    output.push_str(&format!("export interface {name}LazyView {{"));
    for field in &item.fields {
        let ty = if collections
            .iter()
            .any(|candidate| candidate.name == field.name)
        {
            format!(
                "LazyCollection<{}>",
                typescript_lazy_item_type(&field.ty, schema)
            )
        } else {
            typescript_lazy_view_type(&field.ty, schema)
        };
        output.push_str(&format!(
            " {}{}: {};",
            field.name,
            if field.guard.is_some() { "?" } else { "" },
            ty
        ));
    }
    output.push_str(" }\n");
    output.push_str(&format!("function read_{lower}_lazy_view(d: WireDecoder, wire: Uint8Array): {name}LazyView {{ const value = {{}} as {name}LazyView; cid(d, {name}CID);"));
    for field in &item.fields {
        let lhs = format!("value.{}", field.name);
        let is_collection = collections
            .iter()
            .any(|candidate| candidate.name == field.name);
        let decode_collection = if is_collection {
            let item_ty = match &field.ty {
                Type::Vec(item) => item.as_ref(),
                Type::Map(_, _) => &field.ty,
                _ => unreachable!(),
            };
            let mut item_expression = String::new();
            if let Type::Map(key, value) = &field.ty {
                item_expression.push_str("({ key: ");
                typescript_decode_lazy_expression(key, schema, &mut item_expression);
                item_expression.push_str(", value: ");
                typescript_decode_lazy_expression(value, schema, &mut item_expression);
                item_expression.push_str(" })");
            } else {
                typescript_decode_lazy_expression(item_ty, schema, &mut item_expression);
            }
            let callback_expression = item_expression
                .replace("d.", "itemDecoder.")
                .replace("(d)", "(itemDecoder)")
                .replace("(d, wire)", "(itemDecoder, wire)");
            let scan_expression = item_expression.replace("itemDecoder.", "d.");
            let field_id = field.name.replace('_', "");
            let scan = if let Type::Map(key, value) = &field.ty {
                let mut key_expr = String::new();
                typescript_decode_lazy_expression(key, schema, &mut key_expr);
                let mut value_expr = String::new();
                typescript_decode_lazy_expression(value, schema, &mut value_expr);
                let key_compare = if matches!(key.as_ref(), Type::Primitive(name) if name == "String")
                    || is_bytes_type(key)
                {
                    "wireBytesCompare(previousKey, key)"
                } else {
                    "previousKey >= key ? 0 : -1"
                };
                format!(
                    "const key = {key_expr}; if (previousKey !== undefined && {key_compare} >= 0) throw new Error('map keys are not strictly sorted'); previousKey = key; {value_expr};"
                )
            } else {
                scan_expression
            };
            let previous = if matches!(&field.ty, Type::Map(_, _)) {
                let key = match &field.ty {
                    Type::Map(key, _) => key,
                    _ => unreachable!(),
                };
                format!(
                    " let previousKey: {} | undefined;",
                    typescript_lazy_view_type(key, schema)
                )
            } else {
                String::new()
            };
            format!(
                " const {field_id}Count = d.varint();{previous} const {field_id}Start = d.position(); for (let i = 0; i < {field_id}Count; i++) {{ {scan} }} {lhs} = new LazyCollection(wire, {field_id}Start, {field_id}Count, (itemDecoder: WireDecoder) => {callback_expression});"
            )
        } else {
            String::new()
        };
        if let Some(guard) = &field.guard {
            let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!(
                " if ((value.{} & (1 << {})) !== 0) {{",
                owner,
                ts_guard_bit(schema, owner, bit)
            ));
            if is_collection {
                output.push_str(&decode_collection);
            } else {
                typescript_decode_lazy_field_type(&field.ty, &lhs, schema, output);
            }
            output.push_str(" }");
        } else if is_collection {
            output.push_str(&decode_collection);
        } else {
            typescript_decode_lazy_field_type(&field.ty, &lhs, schema, output);
        }
    }
    output.push_str(&format!(" return value; }}\nexport function decode{name}LazyView(wire: Uint8Array): {name}LazyView {{ const d = new WireDecoder(wire); const value = read_{lower}_lazy_view(d, wire); d.done(); return value; }}\nexport function borrow{name}LazyView(wire: Uint8Array): BorrowedPacket<{name}LazyView> {{ return new BorrowedPacket(wire, decode{name}LazyView(wire)); }}\n\n"));
}

fn generate_typescript_view_enum(item: &crate::Enum, schema: &Schema, output: &mut String) {
    let name = &item.name;
    let lower = name.to_ascii_lowercase();
    let variants = item
        .variants
        .iter()
        .map(|v| {
            let fields = v
                .fields
                .iter()
                .map(|f| format!("{}: {}", f.name, typescript_view_type(&f.ty, schema)))
                .collect::<Vec<_>>()
                .join("; ");
            format!("{{ {}: {{ {} }} }}", v.name, fields)
        })
        .collect::<Vec<_>>()
        .join(" | ");
    output.push_str(&format!("export type {name}View = {variants};\nfunction read_{lower}_view(d: WireDecoder): {name}View {{ const c = d.take(8);"));
    for variant in &item.variants {
        let cid = variant
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, variant, schema));
        output.push_str(&format!(
            " if (c.every((x, i) => x === hex(\"{}\")[i])) return {{ {}: {{",
            cid, variant.name
        ));
        for field in &variant.fields {
            output.push_str(&format!(" {}: ", field.name));
            typescript_decode_view_expression(&field.ty, schema, output);
            output.push(',');
        }
        output.push_str(" } };");
    }
    output.push_str(" throw new Error('unknown constructor'); }\n");
    output.push_str(&format!("export function decode{name}View(wire: Uint8Array): {name}View {{ const d = new WireDecoder(wire); const value = read_{lower}_view(d); d.done(); return value; }}\nexport function borrow{name}View(wire: Uint8Array): BorrowedPacket<{name}View> {{ return new BorrowedPacket(wire, decode{name}View(wire)); }}\n\n"));
    generate_typescript_lazy_view_enum(item, schema, output);
}

fn generate_typescript_lazy_view_enum(item: &crate::Enum, schema: &Schema, output: &mut String) {
    let name = &item.name;
    let lower = name.to_ascii_lowercase();
    let variants = item
        .variants
        .iter()
        .map(|variant| {
            let fields = variant
                .fields
                .iter()
                .map(|field| {
                    let is_collection = matches!(field.ty, Type::Vec(_) | Type::Map(_, _))
                        && !is_bytes_type(&field.ty);
                    let ty = if is_collection {
                        format!(
                            "LazyCollection<{}>",
                            typescript_lazy_item_type(&field.ty, schema)
                        )
                    } else {
                        typescript_lazy_view_type(&field.ty, schema)
                    };
                    format!("{}: {}", field.name, ty)
                })
                .collect::<Vec<_>>()
                .join("; ");
            format!("{{ {}: {{ {} }} }}", variant.name, fields)
        })
        .collect::<Vec<_>>()
        .join(" | ");
    output.push_str(&format!("export type {name}LazyView = {variants};\nfunction read_{lower}_lazy_view(d: WireDecoder, wire: Uint8Array): {name}LazyView {{ const c = d.take(8);"));
    for variant in &item.variants {
        let cid = variant
            .cid
            .clone()
            .unwrap_or_else(|| variant_cid_with_schema(item, variant, schema));
        output.push_str(&format!(
            " if (c.every((x, i) => x === hex(\"{}\")[i])) return {{ {}: {{",
            cid, variant.name
        ));
        for field in &variant.fields {
            output.push_str(&format!(" {}: ", field.name));
            let is_collection =
                matches!(field.ty, Type::Vec(_) | Type::Map(_, _)) && !is_bytes_type(&field.ty);
            if !is_collection {
                let mut expr = String::new();
                typescript_decode_lazy_expression(&field.ty, schema, &mut expr);
                output.push_str(&expr);
            } else {
                let field_id = format!(
                    "{}{}",
                    variant.name.to_ascii_lowercase(),
                    field.name.replace('_', "")
                );
                let mut item_expression = String::new();
                if let Type::Map(key, value) = &field.ty {
                    item_expression.push_str("({ key: ");
                    typescript_decode_lazy_expression(key, schema, &mut item_expression);
                    item_expression.push_str(", value: ");
                    typescript_decode_lazy_expression(value, schema, &mut item_expression);
                    item_expression.push_str(" })");
                } else if let Type::Vec(item) = &field.ty {
                    typescript_decode_lazy_expression(item, schema, &mut item_expression);
                }
                let callback_expression = item_expression
                    .replace("d.", "itemDecoder.")
                    .replace("(d)", "(itemDecoder)")
                    .replace("(d, wire)", "(itemDecoder, wire)");
                let scan_expression = item_expression.replace("itemDecoder.", "d.");
                let scan = if let Type::Map(key, value) = &field.ty {
                    let mut key_expr = String::new();
                    typescript_decode_lazy_expression(key, schema, &mut key_expr);
                    let mut value_expr = String::new();
                    typescript_decode_lazy_expression(value, schema, &mut value_expr);
                    let key_compare = if matches!(key.as_ref(), Type::Primitive(name) if name == "String")
                        || is_bytes_type(key)
                    {
                        "wireBytesCompare(previousKey, key)"
                    } else {
                        "previousKey >= key ? 0 : -1"
                    };
                    format!(
                        "const key = {key_expr}; if (previousKey !== undefined && {key_compare} >= 0) throw new Error('map keys are not strictly sorted'); previousKey = key; {value_expr};"
                    )
                } else {
                    scan_expression
                };
                let previous = if let Type::Map(key, _) = &field.ty {
                    format!(
                        " let previousKey: {} | undefined;",
                        typescript_lazy_view_type(key, schema)
                    )
                } else {
                    String::new()
                };
                let item_ty = typescript_lazy_item_type(&field.ty, schema);
                output.push_str(&format!(
                    "(() => {{ const {field_id}Count = d.varint();{previous} const {field_id}Start = d.position(); for (let i = 0; i < {field_id}Count; i++) {{ {scan} }} return new LazyCollection(wire, {field_id}Start, {field_id}Count, (itemDecoder: WireDecoder): {item_ty} => {callback_expression}); }})()"
                ));
            }
            output.push(',');
        }
        output.push_str(" } };");
    }
    output.push_str(&format!(" throw new Error('unknown constructor'); }}\nexport function decode{name}LazyView(wire: Uint8Array): {name}LazyView {{ const d = new WireDecoder(wire); const value = read_{lower}_lazy_view(d, wire); d.done(); return value; }}\nexport function borrow{name}LazyView(wire: Uint8Array): BorrowedPacket<{name}LazyView> {{ return new BorrowedPacket(wire, decode{name}LazyView(wire)); }}\n\n"));
}

fn typescript_decode_view_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!(" {} = d.take({length});", lhs));
        return;
    }
    if is_bytes_type(ty) || matches!(ty, Type::Primitive(name) if name == "String") {
        out.push_str(&format!(" {} = d.bytes();", lhs));
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str(" { const tag = d.u8(); if (tag === 1) {");
            typescript_decode_view_type(item, lhs, schema, out);
            out.push_str(&format!(" }} else if (tag === 0) {{ {} = undefined; }} else throw new Error('invalid optional marker'); }}", lhs));
        }
        Type::Primitive(name) if schema.items.iter().any(|i| item_name(i) == name) => {
            let is_view = typescript_named_view(name, schema);
            out.push_str(&format!(
                " {} = read_{}{}(d);",
                lhs,
                name.to_ascii_lowercase(),
                if is_view { "_view" } else { "" }
            ));
        }
        Type::Primitive(name) => {
            out.push_str(&format!(" {} = d.{}();", lhs, typescript_wire_method(name)))
        }
        Type::Vec(item) => {
            out.push_str(&format!(
                " {} = Array.from({{ length: d.varint() }}, () => ",
                lhs
            ));
            typescript_decode_view_expression(item, schema, out);
            out.push_str(");");
        }
        Type::Map(key, value) => {
            let key_type = typescript_view_type(key, schema);
            let mut key_expression = String::new();
            typescript_decode_view_expression(key, schema, &mut key_expression);
            let mut value_expression = String::new();
            typescript_decode_view_expression(value, schema, &mut value_expression);
            let compare = if matches!(key.as_ref(), Type::Primitive(name) if name == "String")
                || is_bytes_type(key)
            {
                "wireBytesCompare(previousKey, key)"
            } else {
                "previousKey >= key ? 0 : -1"
            };
            out.push_str(&format!(
                " {{ const count = d.varint(); let previousKey: {} | undefined; {} = Array.from({{ length: count }}, () => {{ const key = {}; if (previousKey !== undefined && {} >= 0) throw new Error('map keys are not strictly sorted'); previousKey = key; return {{ key, value: {} }}; }}); }}",
                key_type, lhs, key_expression, compare, value_expression
            ));
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn typescript_decode_view_expression(ty: &Type, schema: &Schema, out: &mut String) {
    if let Type::FixedBytes(length) = ty {
        out.push_str(&format!("d.take({length})"));
        return;
    }
    if is_bytes_type(ty) || matches!(ty, Type::Primitive(name) if name == "String") {
        out.push_str("d.bytes()");
        return;
    }
    match ty {
        Type::Optional(item) => {
            out.push_str("(()=>{const tag=d.u8();if(tag===0)return undefined;if(tag!==1)throw new Error('invalid optional marker');return ");
            typescript_decode_view_expression(item, schema, out);
            out.push_str(";})()");
        }
        Type::Primitive(name) if schema.items.iter().any(|i| item_name(i) == name) => {
            let is_view = typescript_named_view(name, schema);
            out.push_str(&format!(
                "read_{}{}(d)",
                name.to_ascii_lowercase(),
                if is_view { "_view" } else { "" }
            ));
        }
        Type::Primitive(name) => out.push_str(&format!("d.{}()", typescript_wire_method(name))),
        Type::Vec(item) => {
            out.push_str("Array.from({ length: d.varint() }, () => ");
            typescript_decode_view_expression(item, schema, out);
            out.push(')');
        }
        Type::Map(key, value) => {
            out.push_str("Array.from({ length: d.varint() }, () => ({ key: ");
            typescript_decode_view_expression(key, schema, out);
            out.push_str(", value: ");
            typescript_decode_view_expression(value, schema, out);
            out.push_str(" }))");
        }
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn typescript_named_view(name: &str, schema: &Schema) -> bool {
    match schema.items.iter().find(|i| item_name(i) == name) {
        Some(Item::Struct(_)) => true,
        Some(Item::Enum(en)) => !en.variants.iter().all(|v| v.fields.is_empty()),
        _ => false,
    }
}

pub fn generate_python_binding(schema: &Schema) -> String {
    let mut output = format!(
        "# @generated by typikon; Python facade for Layer {}.\nfrom __future__ import annotations\n\nfrom typing import Any\n\n",
        schema.version
    );
    output.push_str("from typikon_python import ");
    output.push_str(
        &schema
            .items
            .iter()
            .map(|item| {
                let function_name = snake_case(item_name(item));
                format!(
                    "encode_{function_name} as _native_encode_{function_name}, decode_{function_name} as _native_decode_{function_name}, validate_borrowed_{function_name} as _native_validate_borrowed_{function_name}"
                )
            })
            .collect::<Vec<_>>()
            .join(", "),
    );
    output.push_str(&format!("\n\nLAYER = {}\n\nclass BorrowedPacket:\n    __slots__ = (\"_wire\", \"type_name\")\n\n    def __init__(self, wire: bytes, type_name: str) -> None:\n        self._wire = memoryview(wire)\n        self.type_name = type_name\n\n    @property\n    def wire(self) -> memoryview:\n        return self._wire\n\n", schema.version));
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(item_name(item));
        output.push_str(&format!(
            "def encode_{function_name}(value: Any) -> bytes:\n    return _native_encode_{function_name}(value)\n\ndef decode_{function_name}(wire: bytes) -> Any:\n    return _native_decode_{function_name}(wire)\n\ndef validate_borrowed_{function_name}(wire: bytes) -> None:\n    _native_validate_borrowed_{function_name}(wire)\n\ndef borrowed_{function_name}(wire: bytes) -> memoryview:\n    \"\"\"Validate and retain the caller-owned packet without copying it.\"\"\"\n    validate_borrowed_{function_name}(wire)\n    return memoryview(wire)\n\ndef borrowed_packet_{function_name}(wire: bytes) -> BorrowedPacket:\n    \"\"\"Return an owner object that keeps the packet backing storage alive.\"\"\"\n    validate_borrowed_{function_name}(wire)\n    return BorrowedPacket(wire, \"{name}\")\n\n"
        ));
    }
    output.push_str("__all__ = [\"LAYER\"");
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        output.push_str(&format!(
            ", \"{name}\", \"encode_{function_name}\", \"decode_{function_name}\", \"validate_borrowed_{function_name}\", \"borrowed_{function_name}\", \"borrowed_packet_{function_name}\""
        ));
    }
    output.push_str("]\n");
    output
}

fn item_name(item: &Item) -> &str {
    match item {
        Item::Alias(item) => &item.name,
        Item::Struct(item) => &item.name,
        Item::Enum(item) => &item.name,
        Item::Flags(item) => &item.name,
    }
}

fn go_primitive(name: &str) -> &str {
    match name {
        "u8" => "uint8",
        "u16" => "uint16",
        "u32" => "uint32",
        "u64" => "uint64",
        "u128" => "string",
        "i8" => "int8",
        "i16" => "int16",
        "i32" => "int32",
        "i64" => "int64",
        "i128" => "string",
        "bool" => "bool",
        "f32" => "float32",
        "f64" => "float64",
        "String" => "string",
        _ => "[]byte",
    }
}

fn typescript_item_type(item: &Item, schema: &Schema) -> String {
    match item {
        Item::Alias(item) => format!(
            "export type {} = {};",
            item.name,
            typescript_type(&item.ty, schema)
        ),
        Item::Flags(item) => format!("export type {} = number;", item.name),
        Item::Enum(item)
            if item
                .variants
                .iter()
                .all(|variant| variant.fields.is_empty()) =>
        {
            let variants = item
                .variants
                .iter()
                .map(|variant| format!("\"{}\"", variant.name))
                .collect::<Vec<_>>()
                .join(" | ");
            format!("export type {} = {};", item.name, variants)
        }
        Item::Enum(item) => {
            let variants = item
                .variants
                .iter()
                .map(|variant| {
                    if variant.fields.is_empty() {
                        format!("\"{}\"", variant.name)
                    } else {
                        let fields = variant
                            .fields
                            .iter()
                            .map(|field| {
                                format!("{}: {}", field.name, typescript_type(&field.ty, schema))
                            })
                            .collect::<Vec<_>>()
                            .join("; ");
                        format!("{{ {}: {{ {} }} }}", variant.name, fields)
                    }
                })
                .collect::<Vec<_>>()
                .join(" | ");
            format!("export type {} = {};", item.name, variants)
        }
        Item::Struct(_) => unreachable!("structs are emitted separately"),
    }
}

fn pascal_case(name: &str) -> String {
    name.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

fn go_type(ty: &Type, schema: &Schema) -> String {
    if let Type::FixedBytes(length) = ty {
        return format!("[{length}]byte");
    }
    if is_bytes_type(ty) {
        return "[]byte".into();
    }
    match ty {
        Type::Optional(item) => format!("*{}", go_type(item, schema)),
        Type::Primitive(name) => match name.as_str() {
            "String" => "string".into(),
            "bool" => "bool".into(),
            "f32" => "float32".into(),
            "f64" => "float64".into(),
            "u8" => "uint8".into(),
            "u16" => "uint16".into(),
            "u32" => "uint32".into(),
            "u64" => "uint64".into(),
            "u128" => "string".into(),
            "i8" => "int8".into(),
            "i16" => "int16".into(),
            "i32" => "int32".into(),
            "i64" => "int64".into(),
            "i128" => "string".into(),
            _ if schema.items.iter().any(|item| item_name(item) == name) => name.clone(),
            _ => "[]byte".into(),
        },
        Type::Vec(item) => format!("[]{}", go_type(item, schema)),
        Type::Map(_, value) => format!("map[string]{}", go_type(value, schema)),
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn typescript_type(ty: &Type, schema: &Schema) -> String {
    if let Type::FixedBytes(_) = ty {
        return "Uint8Array".into();
    }
    if is_bytes_type(ty) {
        return "Uint8Array".into();
    }
    match ty {
        Type::Optional(item) => format!("{} | undefined", typescript_type(item, schema)),
        Type::Primitive(name) => match name.as_str() {
            "String" => "string".into(),
            "bool" => "boolean".into(),
            n if n.starts_with('f') => "number".into(),
            "u64" | "i64" => "bigint".into(),
            n if n.starts_with('u') || n.starts_with('i') => "number".into(),
            _ if schema.items.iter().any(|item| item_name(item) == name) => name.clone(),
            _ => "unknown".into(),
        },
        Type::Vec(item) => format!("Array<{}>", typescript_type(item, schema)),
        Type::Map(_, value) => format!("Record<string, {}>", typescript_type(value, schema)),
        Type::FixedBytes(_) => unreachable!("fixed bytes handled above"),
    }
}

fn is_bytes_type(ty: &Type) -> bool {
    matches!(ty, Type::Vec(item) if matches!(item.as_ref(), Type::Primitive(name) if name == "u8"))
}

#[allow(clippy::if_same_then_else)]
pub fn generate_bridge(schema: &Schema, native_file: &str, kind: BridgeKind) -> String {
    let layer = schema.version;
    let module = native_file.trim_end_matches(".rs").replace(['-', '.'], "_");
    let language = match kind {
        BridgeKind::Python => "Python / PyO3",
        BridgeKind::Go => "Go / cgo",
        BridgeKind::TypeScript => "TypeScript / Node-API",
    };
    if matches!(kind, BridgeKind::Go) {
        return generate_go_validation_bridge(schema, native_file, layer, &module, language);
    }
    let mut output = if matches!(kind, BridgeKind::Python) {
        format!(
            "// @generated by typikon; {language} bridge; do not edit.\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n// Layer {layer} uses the native generated module above for all wire operations.\npub const TYPIKON_LAYER: u16 = {layer};\n"
        )
    } else if matches!(kind, BridgeKind::TypeScript) {
        format!(
            "// @generated by typikon; {language} bridge; do not edit.\n\nuse std::slice;\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\npub const TYPIKON_LAYER: u16 = {layer};\n"
        )
    } else {
        format!(
            "// @generated by typikon; {language} bridge; do not edit.\n\nuse std::slice;\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n#[repr(C)]\npub struct TypikonBridgeResult {{ pub status: i32, pub data_ptr: *mut u8, pub data_len: usize, pub data_capacity: usize, pub error_ptr: *mut u8, pub error_len: usize, pub error_capacity: usize }}\nimpl TypikonBridgeResult {{ fn success(data: Vec<u8>) -> Self {{ let (data_ptr, data_len, data_capacity) = data.into_raw_parts(); Self {{ status: 0, data_ptr, data_len, data_capacity, error_ptr: std::ptr::null_mut(), error_len: 0, error_capacity: 0 }} }} fn failure(error: String) -> Self {{ let (error_ptr, error_len, error_capacity) = error.into_bytes().into_raw_parts(); Self {{ status: 1, data_ptr: std::ptr::null_mut(), data_len: 0, data_capacity: 0, error_ptr, error_len, error_capacity }} }} }}\n\npub const TYPIKON_LAYER: u16 = {layer};\n"
        )
    };
    if !matches!(kind, BridgeKind::Python) {
        output.push_str("fn input_bytes(input: *const u8, len: usize) -> Result<&'static [u8], String> { if len > 0 && input.is_null() { return Err(\"null input\".into()); } if len == 0 { Ok(&[]) } else { Ok(unsafe { slice::from_raw_parts(input, len) }) } }\n");
    }
    if !matches!(kind, BridgeKind::Python) {
        for item in &schema.items {
            let (name, is_flags) = match item {
                Item::Alias(item) => (&item.name, false),
                Item::Struct(item) => (&item.name, false),
                Item::Enum(item) => (&item.name, false),
                Item::Flags(item) => (&item.name, true),
            };
            let function_name = snake_case(name);
            let native_name = format!("{module}::{name}");
            if is_flags {
                output.push_str(&format!(
            "pub fn encode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
                output.push_str(&format!(
            "pub fn decode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
            } else {
                let alias_check = match item {
                    Item::Alias(alias) => alias.exact_len.map(|length| format!(" if value.len() != {length} {{ return Err(\"invalid exact length\".into()); }}")),
                    _ => None,
                };
                let check = alias_check.unwrap_or_default();
                output.push_str(&format!(
            "pub fn encode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?;{check} if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
                output.push_str(&format!(
            "pub fn decode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?;{check} if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
            }
            let borrowed = if let Some(view_name) = borrowed_view_name(item, schema) {
                format!(
                    "let _: {module}::{view_name}<'_> = typikon::decode_borrowed_value(bytes).map_err(|error| format!(\"{{error:?}}\"))?;"
                )
            } else {
                format!(
                    "let mut decoder = typikon::Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }}"
                )
            };
            output.push_str(&format!(
                "pub fn validate_borrowed_{function_name}(input: &[u8]) -> Result<(), String> {{ let bytes = input; {borrowed} Ok(()) }}\n#[unsafe(no_mangle)]\npub extern \"C\" fn typikon_{layer}_{function_name}_validate_borrowed(input: *const u8, len: usize) -> i32 {{ let Ok(bytes) = input_bytes(input, len) else {{ return 1 }}; validate_borrowed_{function_name}(bytes).map(|_| 0).unwrap_or(1) }}\n"
            ));
        }
    }
    if matches!(kind, BridgeKind::Python) {
        output.push_str("use pyo3::exceptions::PyValueError;\n");
        for item in &schema.items {
            let name = item_name(item);
            let function_name = snake_case(name);
            let native_name = format!("{module}::{name}");
            let borrowed_name = borrowed_view_name(item, schema)
                .map(|view| format!("{module}::{view}<'_>"))
                .unwrap_or_else(|| native_name.clone());
            let alias_fixed = match item {
                Item::Alias(alias) => fixed_byte_length(&alias.ty, schema),
                _ => None,
            };
            let alias_exact = match item {
                Item::Alias(alias) => alias.exact_len,
                _ => None,
            };
            let (encode_prefix, encode_body, decode_body, decode_value) = if let Some(length) =
                alias_fixed
            {
                (
                    format!("let bytes: Vec<u8> = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?; if bytes.len() != {length} {{ return Err(PyValueError::new_err(\"invalid fixed byte length\")); }} let value: [u8; {length}] = bytes.try_into().map_err(|_| PyValueError::new_err(\"invalid fixed byte length\"))?;"),
                    "typikon::encode_value(&value).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let value: {native_name} = typikon::decode_value(input).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?;"),
                    "&value.to_vec()".to_owned(),
                )
            } else if let Some(length) = alias_exact {
                (
                    format!("let value: {native_name} = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?; if value.len() != {length} {{ return Err(PyValueError::new_err(\"invalid exact length\")); }}"),
                    "typikon::encode_value(&value).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; if value.len() != {length} {{ return Err(PyValueError::new_err(\"invalid exact length\")); }} if !decoder.is_finished() {{ return Err(PyValueError::new_err(\"trailing bytes\")); }}"),
                    "&value".to_owned(),
                )
            } else if matches!(item, Item::Flags(_)) {
                (
                    format!("let value: {native_name} = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?;"),
                    "let mut encoder = typikon::Encoder::new(typikon::DEFAULT_MAX_PACKET_SIZE); typikon::WireCodec::encode(&value, &mut encoder).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))?; encoder.finish().map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; if !decoder.is_finished() {{ return Err(PyValueError::new_err(\"trailing bytes\")); }}"),
                    "&value".to_owned(),
                )
            } else {
                (
                    format!("let value: {native_name} = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?;"),
                    "typikon::TypikonCodec::encode(&value).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let value: {native_name} = typikon::TypikonCodec::decode(input).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?;"),
                    "&value".to_owned(),
                )
            };
            output.push_str(&format!(
                "#[pyfunction]\nfn encode_{function_name}(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {{ {encode_prefix} {encode_body} }}\n#[pyfunction]\nfn decode_{function_name}(py: Python<'_>, input: &[u8]) -> PyResult<Py<PyAny>> {{ {decode_body} pythonize::pythonize(py, {decode_value}).map(|value| value.unbind()).map_err(|error| PyValueError::new_err(error.to_string())) }}\n#[pyfunction]\nfn validate_borrowed_{function_name}(input: &[u8]) -> PyResult<()> {{ typikon::decode_borrowed_value::<{borrowed_name}>(input).map(|_| ()).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\"))) }}\n"
            ));
        }
        output.push_str(
            "pub fn register_typikon_python(module: &Bound<'_, PyModule>) -> PyResult<()> {\n",
        );
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!("    module.add_function(wrap_pyfunction!(encode_{function_name}, module)?)?;\n    module.add_function(wrap_pyfunction!(decode_{function_name}, module)?)?;\n    module.add_function(wrap_pyfunction!(validate_borrowed_{function_name}, module)?)?;\n"));
        }
        output.push_str("    Ok(())\n}\n");
    }
    if matches!(kind, BridgeKind::TypeScript) {
        output.push_str(
            "use napi::bindgen_prelude::Buffer;\nuse napi_derive::napi;\n\n#[napi]\npub fn abi_version() -> u16 { typikon::ffi_abi_version() }\n\n#[napi]\npub fn negotiate_layer(requested: u16, supported: Vec<u16>) -> napi::Result<u16> { typikon::LayerSupport::new(supported).negotiate(requested).map_err(|error| napi::Error::from_reason(format!(\"unsupported Layer {}\", error.requested))) }\n\nfn check_layer(layer: u16) -> napi::Result<()> { if layer == TYPIKON_LAYER { Ok(()) } else { Err(napi::Error::from_reason(format!(\"unsupported Layer {}\", layer))) } }\n\n#[napi]\npub fn encode_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<Buffer> { check_layer(layer)?; match type_name.as_str() {\n",
        );
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => encode_binary_{}(&input).map(Buffer::from).map_err(napi::Error::from_reason),\n",
                function_name, function_name
            ));
        }
        output.push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n\n#[napi]\npub fn decode_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<Buffer> { check_layer(layer)?; match type_name.as_str() {\n");
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => decode_binary_{}(&input).map(Buffer::from).map_err(napi::Error::from_reason),\n",
                function_name, function_name
            ));
        }
        output
            .push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n");
        output.push_str("\n#[napi]\npub fn validate_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<()> { check_layer(layer)?; match type_name.as_str() {\n");
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => validate_borrowed_{}(&input).map_err(|error| napi::Error::from_reason(format!(\"{{error:?}}\"))),\n",
                function_name, function_name
            ));
        }
        output
            .push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n");
        output.push_str("\n#[napi]\npub fn borrow_binary(layer: u16, type_name: String, input: Buffer) -> napi::Result<Buffer> { check_layer(layer)?; match type_name.as_str() {\n");
        for item in &schema.items {
            let function_name = snake_case(item_name(item));
            output.push_str(&format!(
                "        \"{}\" => {{ validate_borrowed_{}(&input).map_err(|error| napi::Error::from_reason(format!(\"{{error:?}}\")))?; Ok(input) }},\n",
                function_name, function_name
            ));
        }
        output
            .push_str("        _ => Err(napi::Error::from_reason(\"unknown schema type\")), } }\n");
    }
    output
}

fn generate_go_validation_bridge(
    schema: &Schema,
    native_file: &str,
    layer: u16,
    module: &str,
    language: &str,
) -> String {
    let mut output = format!(
        "// @generated by typikon; {language} bridge; do not edit.\n\nuse std::slice;\n\n#[allow(dead_code)]\n#[path = \"{native_file}\"]\nmod {module};\n\n#[repr(C)]\npub struct TypikonBridgeResult {{ pub status: i32, pub data_ptr: *mut u8, pub data_len: usize, pub data_capacity: usize, pub error_ptr: *mut u8, pub error_len: usize, pub error_capacity: usize }}\n\n pub const TYPIKON_LAYER: u16 = {layer};\nfn input_bytes(input: *const u8, len: usize) -> Result<&'static [u8], String> {{ if len > 0 && input.is_null() {{ return Err(\"null input\".into()); }} if len == 0 {{ Ok(&[]) }} else {{ Ok(unsafe {{ slice::from_raw_parts(input, len) }}) }} }}\n",
    );
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        let borrowed = if let Some(view_name) = borrowed_view_name(item, schema) {
            format!(
                "let _: {module}::{view_name}<'_> = typikon::decode_borrowed_value(bytes).map_err(|error| format!(\"{{error:?}}\"))?;"
            )
        } else {
            let native_name = format!("{module}::{name}");
            format!(
                "let mut decoder = typikon::Decoder::new(bytes, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }}"
            )
        };
        output.push_str(&format!("pub fn validate_borrowed_{function_name}(input: &[u8]) -> Result<(), String> {{ let bytes = input; {borrowed} Ok(()) }}\n#[unsafe(no_mangle)]\npub extern \"C\" fn typikon_{layer}_{function_name}_validate_borrowed(input: *const u8, len: usize) -> i32 {{ let Ok(bytes) = input_bytes(input, len) else {{ return 1 }}; validate_borrowed_{function_name}(bytes).map(|_| 0).unwrap_or(1) }}\n"));
    }
    output
}

fn snake_case(name: &str) -> String {
    name.chars()
        .enumerate()
        .map(|(index, character)| {
            if character.is_ascii_uppercase() && index > 0 {
                format!("_{}", character.to_ascii_lowercase())
            } else {
                character.to_ascii_lowercase().to_string()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse_schema;

    #[test]
    fn bridges_reference_the_same_native_backend() {
        let schema = parse_schema("#[version(10)] struct Message { id: u64, }").unwrap();
        for kind in BridgeKind::ALL {
            let source = generate_bridge(&schema, "chat-10.rs", kind);
            assert!(source.contains("mod chat_10"));
            assert!(source.contains("TYPIKON_LAYER: u16 = 10"));
            if matches!(kind, BridgeKind::Python) {
                assert!(source.contains("fn encode_message"));
            } else {
                assert!(source.contains("validate_borrowed_message"));
                assert!(!source.contains("serde_json"));
            }
        }
        assert_eq!(
            BridgeKind::Python.file_name("chat", 10),
            "python.chat-10.rs"
        );
    }

    #[test]
    fn go_binding_uses_real_go_integer_types() {
        let schema =
            parse_schema("#[version(10)] struct Message { small: u8, id: u64, signed: i32, }")
                .unwrap();
        let source = generate_go_binding(&schema, "typikon-10.h");
        assert!(source.contains("Small uint8"));
        assert!(source.contains("Id uint64"));
        assert!(source.contains("Signed int32"));
        assert!(!source.contains(" Small u8"));
        assert!(!source.contains(" Id u64"));
    }

    #[test]
    fn generated_backends_validate_fixed_and_exact_bytes() {
        let schema = parse_schema(
            "#[version(10)] type ConnectionId = bytes[16]; struct Packet { id: ConnectionId, hash: Vec<u8> #[exact_len(32)], }",
        )
        .unwrap();
        let go = generate_go_binding(&schema, "packet-10.h");
        assert!(go.contains("type ConnectionId = [16]byte"));
        assert!(go.contains("invalid exact byte length"));
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("export type ConnectionId = Uint8Array;"));
        assert!(typescript.contains("invalid exact byte length"));
        let python = generate_bridge(&schema, "packet-10.rs", BridgeKind::Python);
        assert!(python.contains("Packet"));
        assert!(python.contains("register_typikon_python(module"));
        let rust = crate::codegen::generate_rust(&schema);
        assert!(rust.contains("__typikon_fixed_bytes_16"));
        assert!(rust.contains("__typikon_fixed_bytes_16\")]"));
    }

    #[test]
    fn typescript_views_decode_direct_fixed_bytes_in_enum_variants() {
        let schema = parse_schema(
            "#[version(10)] enum Peer { User { user_id: u32, user_ref: bytes[24] }, }",
        )
        .unwrap();
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("user_ref: d.take(24)"));
    }

    #[test]
    fn generated_go_and_typescript_views_cover_borrowable_fields() {
        let schema = parse_schema(
            "#[version(10)] struct User { id: u64, name: String, tags: Vec<String>, } struct Attachment { id: u64, name: String, } enum Event { Created { user: User }, } enum Batch { Items { values: Vec<String>, entries: Map<String, String> }, }",
        )
        .unwrap();
        let go = generate_go_binding(&schema, "chat-10.h");
        assert!(go.contains("type UserView struct"));
        assert!(go.contains("func BorrowUser"));
        assert!(go.contains("func BorrowAttachment"));
        assert!(go.contains("Name []byte"));
        assert!(go.contains("readUserView"));
        assert!(go.contains("BorrowUserLazy"));
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("export interface UserView"));
        assert!(typescript.contains("name: Uint8Array"));
        assert!(typescript.contains("decodeUserView"));
        assert!(typescript.contains("decodeUserLazyView"));
        assert!(typescript.contains("LazyCollection"));
        assert!(typescript.contains("borrowBinary"));
        assert!(typescript.contains("export class BorrowedPacket<T>"));
        assert!(typescript.contains("borrowUserView"));
        assert!(typescript.contains("borrowUserLazyView"));
        assert!(typescript.contains("export type EventView"));
        assert!(typescript.contains("LazyCollection<Uint8Array>"));
        assert!(typescript.contains("itemsvaluesCount"));
        assert!(go.contains("func BorrowBatchLazy"));
        assert!(go.contains("BatchItemsValuesLazyView"));
        assert!(go.contains("BatchItemsEntriesEntry"));
        assert!(go.contains("entry.Key)>=0"));
    }

    #[test]
    fn typescript_preserves_64_bit_integer_precision() {
        let schema =
            parse_schema("#[version(10)] struct Message { unsigned: u64, signed: i64, }").unwrap();
        let generated = generate_typescript_binding(&schema);
        assert!(generated.contains("unsigned: bigint"));
        assert!(generated.contains("signed: bigint"));
        assert!(generated.contains("u64(v: bigint)"));
        assert!(generated.contains("u64(): bigint"));
        assert!(generated.contains("i64(): bigint"));
        assert!(!generated.contains("u64(): number"));
        assert!(!generated.contains("i64(): number"));
    }

    #[test]
    fn generated_backends_derive_guard_bits_from_field_presence() {
        let schema = parse_schema(
            "#[version(10)] #[flags(u16)] enum Flags { HasAvatar = 2, HasBio = 3, } struct User { flags: Flags, #[guard(flags.has_avatar)] avatar: String, #[guard(flags.has_bio)] bio: String, }",
        )
        .unwrap();
        let go = generate_go_binding(&schema, "chat-10.h");
        assert!(go.contains("__typikon_effective_Flags:=v.Flags"));
        assert!(go.contains("v.Avatar!=nil"));
        assert!(go.contains("v.Bio!=nil"));
        assert_eq!(go.matches("__typikon_effective_Flags:=v.Flags").count(), 1);
        assert!(go.contains("encode_flags(e,__typikon_effective_Flags)"));
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("let effective_flags = value.flags"));
        assert!(typescript.contains("value.avatar !== undefined"));
        assert!(typescript.contains("value.bio !== undefined"));
        assert_eq!(
            typescript
                .matches("let effective_flags = value.flags")
                .count(),
            1
        );
        assert!(typescript.contains("write_flags(e, effective_flags)"));
    }

    #[test]
    fn generated_bridges_expose_owned_error_and_data_buffers() {
        let schema = parse_schema("#[version(10)] struct Message { id: u64, }").unwrap();
        let source = generate_bridge(&schema, "chat-10.rs", BridgeKind::Go);
        assert!(source.contains("data_capacity"));
        assert!(source.contains("error_capacity"));
        assert!(source.contains("typikon_10_message_validate_borrowed"));
    }

    #[test]
    fn typescript_bridge_has_node_api_dispatch_for_every_item() {
        let schema = parse_schema(
            "#[version(10)] struct User { id: u64, } enum Event { Created { user: User }, } #[flags(u16)] enum Flags { Ready = 0, }",
        )
        .unwrap();
        let source = generate_bridge(&schema, "chat-10.rs", BridgeKind::TypeScript);
        assert!(source.contains("#[napi]"));
        assert!(source.contains("\"user\" => encode_binary_user"));
        assert!(source.contains("\"event\" => decode_binary_event"));
        assert!(source.contains("\"flags\" => encode_binary_flags"));
        assert!(!source.contains("pub fn encode_json"));
        assert!(!source.contains("pub fn decode_json"));
    }

    #[test]
    fn generated_language_types_preserve_flags_and_enums() {
        let schema = parse_schema(
            "#[version(10)] #[flags(u16)] enum Flags { Ready = 0, } enum Event { Created { id: u64 }, }",
        )
        .unwrap();
        let go = generate_go_binding(&schema, "chat-10.h");
        assert!(go.contains("type Flags uint16"));
        assert!(go.contains("type Event interface{isEvent()}"));
        assert!(!go.contains("encoding/json"));
        let typescript = generate_typescript_binding(&schema);
        assert!(typescript.contains("export type Flags = number;"));
        assert!(typescript.contains("export type Event = { Created:"));
    }

    #[test]
    fn python_facade_wraps_the_pyo3_extension() {
        let schema = parse_schema("#[version(10)] struct User { id: u64, }").unwrap();
        let source = generate_python_binding(&schema);
        assert!(source.contains("from typikon_python import encode_user as _native_encode_user"));
        assert!(source.contains("def encode_user"));
        assert!(source.contains("def decode_user"));
        assert!(source.contains("def borrowed_user"));
        assert!(source.contains("def borrowed_packet_user"));
        assert!(source.contains("return memoryview(wire)"));
        assert!(source.contains("return _native_encode_user(value)"));
        assert!(source.contains("LAYER = 10"));
    }
}
