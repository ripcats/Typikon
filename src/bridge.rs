use crate::codegen::borrowed_view_name;
use crate::fingerprint::{constructor_cid, variant_cid};
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

pub fn generate_go_binding(schema: &Schema, _header_name: &str) -> String {
    generate_go_binding_direct(schema)
}

fn generate_go_binding_direct(schema: &Schema) -> String {
    let mut output = String::from(
        r#"package typikon

import (
    "encoding/binary"
    "fmt"
    "math"
    "sort"
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

"#,
    );
    for item in &schema.items {
        generate_go_item(item, schema, &mut output);
    }
    output
}

fn generate_go_item(item: &Item, schema: &Schema, output: &mut String) {
    let name = item_name(item);
    let function_name = snake_case(name);
    match item {
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
    output.push_str(&format!("func Encode{name}(v {name})([]byte,error){{e:=wireEncoder{{}};encode_{function_name}(&e,v);return e.finish()}}\nfunc Decode{name}(b []byte)({name},error){{d:=wireDecoder{{b:b}};v,e:=decode_{function_name}(&d);if e==nil{{e=d.done()}};return v,e}}\nfunc Validate{name}(b []byte)error{{_,e:=Decode{name}(b);return e}}\n\n"));
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
    let cid = item.cid.clone().unwrap_or_else(|| constructor_cid(item));
    output.push_str(&format!("var {}CID=[]byte{{{}}}\n", name, cid_bytes(&cid)));
    output.push_str(&format!(
        "func encode_{}(e *wireEncoder,v {}){{e.raw({}CID);",
        snake_case(name),
        name,
        name
    ));
    for field in &item.fields {
        let expr = format!("v.{}", pascal_case(&field.name));
        if let Some(guard) = &field.guard {
            let (_, bit) = guard.split_once('.').unwrap_or(("flags", guard));
            output.push_str(&format!("if v.Flags&(1<<{})!=0{{", flag_value(item, bit)));
            go_encode_go_type(&field.ty, &format!("*{}", expr), schema, output);
            output.push_str("};");
        } else {
            go_encode_go_type(&field.ty, &expr, schema, output);
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
            output.push_str(&format!("v.{}=&{};", pascal_case(&field.name), temporary));
            output.push_str("};");
        } else {
            go_decode_go_type(&field.ty, &lhs, schema, output, false);
        }
    }
    output.push_str("return v,e}\n");
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
        let cid = v.cid.clone().unwrap_or_else(|| variant_cid(item, v));
        output.push_str(&format!("case {vn}:e.raw([]byte{{{}}});", cid_bytes(&cid)));
        for f in &v.fields {
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
        let cid = v.cid.clone().unwrap_or_else(|| variant_cid(item, v));
        output.push_str(&format!(
            "case string([]byte{{{}}}):var x {vn};",
            cid_bytes(&cid)
        ));
        for f in &v.fields {
            go_decode_go_type(
                &f.ty,
                &format!("x.{}", pascal_case(&f.name)),
                schema,
                output,
                false,
            );
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
        .and_then(|_| {
            Some(match bit {
                "is_bot" => 0,
                "is_verified" => 1,
                "has_avatar" => 2,
                _ => 0,
            })
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
    match ty {
        Type::Primitive(n) => {
            if schema.items.iter().any(|i| item_name(i) == n) {
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
    }
}
fn go_decode_go_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String, guarded: bool) {
    let _ = guarded;
    match ty {
        Type::Primitive(n) => {
            if schema.items.iter().any(|i| item_name(i) == n) {
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
    }
}

pub fn generate_typescript_binding(schema: &Schema) -> String {
    let mut output = String::from(
        "export interface TypikonNative { encodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; decodeBinary(layer: number, typeName: string, input: Uint8Array): Uint8Array; validateBinary(layer: number, typeName: string, input: Uint8Array): void; }\n\nclass WireEncoder { private b: number[] = []; raw(v: Uint8Array): void { for (const x of v) this.b.push(x); } u8(v: number): void { this.b.push(v & 255); } u16(v: number): void { this.u8(v); this.u8(v >>> 8); } u32(v: number): void { this.u8(v); this.u8(v >>> 8); this.u8(v >>> 16); this.u8(v >>> 24); } u64(v: number): void { let n = BigInt(v); for (let i = 0n; i < 8n; i++) { this.u8(Number(n & 255n)); n >>= 8n; } } i8(v: number): void { this.u8(v); } i16(v: number): void { this.u16(v); } i32(v: number): void { this.u32(v); } i64(v: number): void { this.u64(v); } f32(v: number): void { const x = new DataView(new ArrayBuffer(4)); x.setFloat32(0, v, true); this.u32(x.getUint32(0, true)); } f64(v: number): void { const x = new DataView(new ArrayBuffer(8)); x.setFloat64(0, v, true); this.u64(x.getBigUint64(0, true) as unknown as number); } bool(v: boolean): void { this.u8(v ? 1 : 0); } varint(v: number): void { let n = BigInt(v); while (n >= 128n) { this.u8(Number(n & 127n) | 128); n >>= 7n; } this.u8(Number(n)); } bytes(v: Uint8Array): void { this.varint(v.length); this.raw(v); } string(v: string): void { this.bytes(new TextEncoder().encode(v)); } finish(): Uint8Array { if (this.b.length > 4 * 1024 * 1024) throw new Error('packet exceeds limit'); return Uint8Array.from(this.b); } }\nclass WireDecoder { private p = 0; constructor(private readonly b: Uint8Array) {} take(n: number): Uint8Array { if (n < 0 || this.p > this.b.length - n) throw new Error('truncated wire'); const v = this.b.subarray(this.p, this.p + n); this.p += n; return v; } u8(): number { return this.take(1)[0]; } u16(): number { return this.u8() | (this.u8() << 8); } u32(): number { return (this.u8() | (this.u8() << 8) | (this.u8() << 16) | (this.u8() << 24)) >>> 0; } u64(): number { let n = 0n; for (let i = 0n; i < 8n; i++) n |= BigInt(this.u8()) << (8n * i); return Number(n); } i8(): number { return (this.u8() << 24) >> 24; } i16(): number { const n = this.u16(); return (n << 16) >> 16; } i32(): number { return this.u32() | 0; } i64(): number { return this.u64(); } f32(): number { const x = new DataView(this.take(4).slice().buffer); return x.getFloat32(0, true); } f64(): number { const x = new DataView(this.take(8).slice().buffer); return x.getFloat64(0, true); } bool(): boolean { return this.u8() !== 0; } varint(): number { let n = 0n; for (let i = 0n; i < 10n; i++) { const b = this.u8(); n |= BigInt(b & 127) << (7n * i); if (b < 128) return Number(n); } throw new Error('varint overflow'); } bytes(): Uint8Array { const n = this.varint(); return this.take(n); } string(): string { return new TextDecoder().decode(this.bytes()); } done(): void { if (this.p !== this.b.length) throw new Error('trailing bytes'); } }\nconst cid = (d: WireDecoder, want: Uint8Array): void => { const got = d.take(8); for (let i = 0; i < 8; i++) if (got[i] !== want[i]) throw new Error('invalid constructor ID'); };\nconst hex = (s: string): Uint8Array => Uint8Array.from(s.match(/.{2}/g)!.map(x => parseInt(x, 16)));\n\n",
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
        Item::Struct(st) => {
            let name = &st.name;
            let fn_name = name.to_ascii_lowercase();
            let cid = st.cid.clone().unwrap_or_else(|| constructor_cid(st));
            output.push_str(&format!("const {name}CID = hex(\"{cid}\");\nfunction write_{fn_name}(e: WireEncoder, value: {name}): void {{ e.raw({name}CID);"));
            for field in &st.fields {
                let expr = format!("value.{}", field.name);
                if let Some(guard) = &field.guard {
                    let (owner, bit) = guard.split_once('.').unwrap_or(("flags", guard));
                    output.push_str(&format!(
                        " if ((value.{} & (1 << {})) !== 0) {{",
                        owner,
                        ts_guard_bit(schema, owner, bit)
                    ));
                    typescript_encode_type(&field.ty, &expr, schema, output);
                    output.push_str(" }");
                } else {
                    typescript_encode_type(&field.ty, &expr, schema, output);
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
                    output.push_str(" }");
                } else {
                    typescript_decode_type(&field.ty, &lhs, schema, output);
                }
            }
            output.push_str(" return value; }\n");
            output.push_str(&format!("export function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{fn_name}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{fn_name}(d); d.done(); return value; }}\n\n"));
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
                    " case \"{}\": e.u64({}); break;",
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
                    " case {}: return \"{}\";",
                    v.value.unwrap_or_default(),
                    v.name
                ));
            }
            output.push_str(" default: throw new Error('invalid enum'); } }\n");
            output.push_str(&format!("export function encode{name}(value: {name}): Uint8Array {{ const e = new WireEncoder(); write_{}(e, value); return e.finish(); }}\nexport function decode{name}(wire: Uint8Array): {name} {{ const d = new WireDecoder(wire); const value = read_{}(d); d.done(); return value; }}\n\n", name.to_ascii_lowercase(), name.to_ascii_lowercase()));
        }
        Item::Enum(_) => {}
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
            if item_name(item) == owner {
                if let Item::Flags(flags) = item {
                    flags.bits.iter().find(|x| x.name == bit).map(|x| x.value)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| match bit {
            "is_bot" => 0,
            "is_verified" => 1,
            "has_avatar" => 2,
            _ => 0,
        })
}
fn typescript_encode_type(ty: &Type, expr: &str, schema: &Schema, out: &mut String) {
    match ty {
        Type::Primitive(n) => {
            if schema.items.iter().any(|i| item_name(i) == n) {
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
    }
}
fn typescript_decode_type(ty: &Type, lhs: &str, schema: &Schema, out: &mut String) {
    match ty {
        Type::Primitive(n) => {
            if schema.items.iter().any(|i| item_name(i) == n) {
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
    output.push_str(&format!("\n\nLAYER = {}\n\n", schema.version));
    for item in &schema.items {
        let function_name = snake_case(item_name(item));
        output.push_str(&format!(
            "def encode_{function_name}(value: Any) -> bytes:\n    return _native_encode_{function_name}(value)\n\ndef decode_{function_name}(wire: bytes) -> Any:\n    return _native_decode_{function_name}(wire)\n\ndef validate_borrowed_{function_name}(wire: bytes) -> None:\n    _native_validate_borrowed_{function_name}(wire)\n\n"
        ));
    }
    output.push_str("__all__ = [\"LAYER\"");
    for item in &schema.items {
        let name = item_name(item);
        let function_name = snake_case(name);
        output.push_str(&format!(
            ", \"{name}\", \"encode_{function_name}\", \"decode_{function_name}\", \"validate_borrowed_{function_name}\""
        ));
    }
    output.push_str("]\n");
    output
}

fn item_name(item: &Item) -> &str {
    match item {
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
    match ty {
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
    }
}

fn typescript_type(ty: &Type, schema: &Schema) -> String {
    match ty {
        Type::Primitive(name) => match name.as_str() {
            "String" => "string".into(),
            "bool" => "boolean".into(),
            n if n.starts_with('f') => "number".into(),
            n if n.starts_with('u') || n.starts_with('i') => "number".into(),
            _ if schema.items.iter().any(|item| item_name(item) == name) => name.clone(),
            _ => "unknown".into(),
        },
        Type::Vec(item) => format!("Array<{}>", typescript_type(item, schema)),
        Type::Map(_, value) => format!("Record<string, {}>", typescript_type(value, schema)),
    }
}

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
                output.push_str(&format!(
            "pub fn encode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
        ));
                output.push_str(&format!(
            "pub fn decode_binary_{function_name}(input: &[u8]) -> Result<Vec<u8>, String> {{ let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| format!(\"{{error:?}}\"))?; let _: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| format!(\"{{error:?}}\"))?; if !decoder.is_finished() {{ return Err(\"trailing bytes\".into()); }} Ok(input.to_vec()) }}\n"
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
            let (encode_body, decode_body) = if matches!(item, Item::Flags(_)) {
                (
                    "let mut encoder = typikon::Encoder::new(typikon::DEFAULT_MAX_PACKET_SIZE); typikon::WireCodec::encode(&value, &mut encoder).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))?; encoder.finish().map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let mut decoder = typikon::Decoder::new(input, typikon::DEFAULT_MAX_PACKET_SIZE).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; let value: {native_name} = typikon::WireCodec::decode(&mut decoder).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?; if !decoder.is_finished() {{ return Err(PyValueError::new_err(\"trailing bytes\")); }}"),
                )
            } else {
                (
                    "typikon::TypikonCodec::encode(&value).map_err(|error| PyValueError::new_err(format!(\"{error:?}\")))".to_owned(),
                    format!("let value: {native_name} = typikon::TypikonCodec::decode(input).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\")))?;"),
                )
            };
            output.push_str(&format!(
                "#[pyfunction]\nfn encode_{function_name}(value: &Bound<'_, PyAny>) -> PyResult<Vec<u8>> {{ let value: {native_name} = pythonize::depythonize(value).map_err(|error| PyValueError::new_err(error.to_string()))?; {encode_body} }}\n#[pyfunction]\nfn decode_{function_name}(py: Python<'_>, input: &[u8]) -> PyResult<Py<PyAny>> {{ {decode_body} pythonize::pythonize(py, &value).map(|value| value.unbind()).map_err(|error| PyValueError::new_err(error.to_string())) }}\n#[pyfunction]\nfn validate_borrowed_{function_name}(input: &[u8]) -> PyResult<()> {{ typikon::decode_borrowed_value::<{borrowed_name}>(input).map(|_| ()).map_err(|error| PyValueError::new_err(format!(\"{{error:?}}\"))) }}\n"
            ));
        }
        output.push_str(&format!("pub fn register_typikon_python_{layer}(module: &Bound<'_, PyModule>) -> PyResult<()> {{\n"));
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
        assert!(source.contains("return _native_encode_user(value)"));
        assert!(source.contains("LAYER = 10"));
    }
}
