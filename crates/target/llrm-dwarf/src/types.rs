//! The DIE tree of `Info`: the compile unit, its types, globals and functions.

use llrm_object::debug::{Block, Function, Info, Kind, Language, Location, Reach, Scalar, Type, Variable};
use llrm_object::{Binding, Object, Unsupported};

use crate::die::*;
use crate::refused;

const TAG_ARRAY: u16 = 0x01;
const TAG_ENUMERATION: u16 = 0x04;
const TAG_FORMAL_PARAMETER: u16 = 0x05;
const TAG_LEXICAL_BLOCK: u16 = 0x0b;
const TAG_MEMBER: u16 = 0x0d;
const TAG_POINTER: u16 = 0x0f;
const TAG_REFERENCE: u16 = 0x10;
const TAG_STRUCTURE: u16 = 0x13;
const TAG_SUBROUTINE_TYPE: u16 = 0x15;
const TAG_TYPEDEF: u16 = 0x16;
const TAG_SUBRANGE: u16 = 0x21;
const TAG_BASE: u16 = 0x24;
const TAG_CONST: u16 = 0x26;
const TAG_ENUMERATOR: u16 = 0x28;
const TAG_SUBPROGRAM: u16 = 0x2e;
const TAG_VARIABLE: u16 = 0x34;
const TAG_VOLATILE: u16 = 0x35;

const ATE_BOOLEAN: u8 = 2;
const ATE_FLOAT: u8 = 4;
const ATE_SIGNED: u8 = 5;
const ATE_UNSIGNED: u8 = 7;
const ATE_SIGNED_CHAR: u8 = 6;

/// DW_LANG_C89, and the vendor code for assembly, which a debugger reads no language into.
fn language(one: Language) -> u16 {
    match one {
        Language::C => 0x0001,
        Language::Unknown | Language::Basic | Language::Nib => 0x8001,
    }
}

struct Tree<'a> {
    object: &'a Object,
    info: &'a Info,
    dies: Vec<Die>,
    /// Why each type cannot be written, if it cannot: a type is refused where something uses it.
    bad: Vec<Option<String>>,
}

impl Tree<'_> {
    fn push(&mut self, die: Die) -> usize {
        self.dies.push(die);
        self.dies.len() - 1
    }

    /// The DIE of type `index`, None for void.
    fn ty(&self, index: usize) -> Result<Option<usize>, Unsupported> {
        if let Some(Some(why)) = self.bad.get(index) {
            return refused(why);
        }
        Ok(match self.info.types.get(index) {
            Some(Type::Scalar(Scalar::Void)) | None => None,
            Some(_) => Some(1 + index),
        })
    }

    fn typed(&self, die: &mut Die, index: usize) -> Result<(), Unsupported> {
        if let Some(target) = self.ty(index)? {
            die.attrs.push((AT_TYPE, Value::Ref(target)));
        }
        Ok(())
    }

    fn address(&self, section: usize, offset: usize) -> Result<Value, Unsupported> {
        let (symbol, base) = crate::anchor(self.object, section)?;
        Ok(Value::Addr { symbol, delta: offset as i64 - base as i64 })
    }

    /// Type `index`'s DIE, which sits at `1 + index`.
    fn describe(&mut self, index: usize) -> Result<(), Unsupported> {
        let one = self.info.types[index].clone();
        let mut die = match &one {
            Type::Scalar(Scalar::Void) => return Ok(()),
            Type::Scalar(scalar) => base(*scalar)?,
            Type::FixedString(_) => return refused("a BASIC STRING * n has no DWARF type"),
            Type::Array { element, bytes: Some(bytes) } => {
                let each = self.info.size_of(*element).filter(|&one| one > 0).ok_or_else(|| Unsupported("DWARF: an array of an element with no size".into()))?;
                let mut die = Die::new(TAG_ARRAY);
                self.typed(&mut die, *element)?;
                let mut range = Die::new(TAG_SUBRANGE);
                range.attrs.push((AT_UPPER_BOUND, Value::Udata((u64::from(*bytes) / each).saturating_sub(1))));
                let at = self.push(range);
                die.children.push(at);
                die
            }
            Type::Array { bytes: None, .. } => return refused("a BASIC array is its descriptor's: it has no DWARF type"),
            Type::Struct { name, bytes, fields } => {
                let mut die = Die::new(TAG_STRUCTURE);
                if !name.is_empty() {
                    die.attrs.push((AT_NAME, Value::Str(name.clone())));
                }
                die.attrs.push((AT_BYTE_SIZE, Value::Udata(u64::from(*bytes))));
                for field in fields {
                    let mut member = Die::new(TAG_MEMBER);
                    member.attrs.push((AT_NAME, Value::Str(field.name.clone())));
                    self.typed(&mut member, field.r#type)?;
                    match field.bits {
                        None => member.attrs.push((AT_DATA_MEMBER_LOCATION, Value::Udata(u64::from(field.offset)))),
                        Some((start, width)) => {
                            member.attrs.push((AT_BIT_SIZE, Value::Udata(u64::from(width))));
                            member.attrs.push((AT_DATA_BIT_OFFSET, Value::Udata(u64::from(field.offset) * 8 + u64::from(start))));
                        }
                    }
                    let at = self.push(member);
                    die.children.push(at);
                }
                die
            }
            Type::Enum { name, underlying, enumerators } => {
                let mut die = Die::new(TAG_ENUMERATION);
                if !name.is_empty() {
                    die.attrs.push((AT_NAME, Value::Str(name.clone())));
                }
                if let Some(bytes) = self.info.size_of(*underlying) {
                    die.attrs.push((AT_BYTE_SIZE, Value::Udata(bytes)));
                }
                self.typed(&mut die, *underlying)?;
                for one in enumerators {
                    let mut each = Die::new(TAG_ENUMERATOR);
                    each.attrs.push((AT_NAME, Value::Str(one.name.clone())));
                    each.attrs.push((AT_CONST_VALUE, Value::Sdata(one.value)));
                    let at = self.push(each);
                    die.children.push(at);
                }
                die
            }
            Type::Pointer { target, bytes, reach } => {
                if *reach != Reach::Near {
                    return refused("a far or huge pointer has no DWARF type");
                }
                let mut die = Die::new(TAG_POINTER);
                die.attrs.push((AT_BYTE_SIZE, Value::U8(*bytes)));
                self.typed(&mut die, *target)?;
                die
            }
            Type::Reference(target) => {
                let mut die = Die::new(TAG_REFERENCE);
                die.attrs.push((AT_BYTE_SIZE, Value::U8(self.object.arch.bits() as u8 / 8)));
                self.typed(&mut die, *target)?;
                die
            }
            Type::Typedef { name, target } => {
                let mut die = Die::new(TAG_TYPEDEF);
                die.attrs.push((AT_NAME, Value::Str(name.clone())));
                self.typed(&mut die, *target)?;
                die
            }
            Type::Qualified { target, constant, volatile } => {
                // const over volatile over the target; the outer one is this type's DIE.
                let tags: Vec<u16> = [(*constant, TAG_CONST), (*volatile, TAG_VOLATILE)].into_iter().filter(|(on, _)| *on).map(|(_, tag)| tag).collect();
                let Some((&first, rest)) = tags.split_first() else { return refused("a qualified type with no qualifier") };
                let mut inner = self.ty(*target)?;
                for &tag in rest.iter().rev() {
                    let mut die = Die::new(tag);
                    if let Some(at) = inner {
                        die.attrs.push((AT_TYPE, Value::Ref(at)));
                    }
                    inner = Some(self.push(die));
                }
                let mut die = Die::new(first);
                if let Some(at) = inner {
                    die.attrs.push((AT_TYPE, Value::Ref(at)));
                }
                die
            }
            Type::Procedure { result, parameters, .. } => {
                let mut die = Die::new(TAG_SUBROUTINE_TYPE);
                die.attrs.push((AT_PROTOTYPED, Value::Flag));
                if let Some(result) = result {
                    self.typed(&mut die, *result)?;
                }
                for &parameter in parameters {
                    let mut one = Die::new(TAG_FORMAL_PARAMETER);
                    self.typed(&mut one, parameter)?;
                    let at = self.push(one);
                    die.children.push(at);
                }
                die
            }
        };
        die.attrs.shrink_to_fit();
        self.dies[1 + index] = die;
        Ok(())
    }

    /// The expression of where `location` is.
    fn location(&self, location: &Location) -> Result<Value, Unsupported> {
        Ok(match location {
            Location::Frame { disp } => {
                let mut bytes = vec![0x91];
                bytes.extend(crate::sleb(*disp));
                Value::Expr(bytes)
            }
            Location::Register(name) => Value::Expr(self.register(name)?),
            Location::Static { symbol, disp } => Value::ExprAddr { symbol: *symbol, delta: *disp },
            Location::List(_) => return refused("a location list is not written yet"),
        })
    }

    /// `DW_OP_reg`, of the register `name`.
    fn register(&self, name: &str) -> Result<Vec<u8>, Unsupported> {
        let number = self.info.registers.iter().find(|one| one.name == name).and_then(|one| one.dwarf);
        match number {
            Some(number) if number < 32 => Ok(vec![0x50 + number as u8]),
            Some(number) => Ok([vec![0x90], crate::uleb(u64::from(number))].concat()),
            None => refused(format!("register {name} has no DWARF number")),
        }
    }

    fn variable(&mut self, one: &Variable, global: bool) -> Result<usize, Unsupported> {
        let mut die = Die::new(if one.kind == Kind::Parameter { TAG_FORMAL_PARAMETER } else { TAG_VARIABLE });
        die.attrs.push((AT_NAME, Value::Str(one.name.clone())));
        self.typed(&mut die, one.r#type)?;
        if let (true, Location::Static { symbol, .. }) = (global, &one.location) {
            if self.object.symbols[*symbol].binding == Binding::Public {
                die.attrs.push((AT_EXTERNAL, Value::Flag));
            }
        }
        die.attrs.push((AT_LOCATION, self.location(&one.location)?));
        Ok(self.push(die))
    }

    fn scope(&mut self, die: &mut Die, variables: &[Variable], blocks: &[Block]) -> Result<(), Unsupported> {
        for one in variables {
            let at = self.variable(one, false)?;
            die.children.push(at);
        }
        for block in blocks {
            let [range] = block.ranges[..] else { return refused("a block of several ranges is not written yet") };
            let mut inner = Die::new(TAG_LEXICAL_BLOCK);
            inner.attrs.push((AT_LOW_PC, self.address(range.section, range.offset)?));
            inner.attrs.push((AT_HIGH_PC, Value::Len(range.length as u32)));
            self.scope(&mut inner, &block.variables, &block.blocks)?;
            let at = self.push(inner);
            die.children.push(at);
        }
        Ok(())
    }

    fn function(&mut self, one: &Function) -> Result<usize, Unsupported> {
        let [range] = one.ranges[..] else { return refused(format!("function {} has {} ranges: one is written", one.name, one.ranges.len())) };
        let mut die = Die::new(TAG_SUBPROGRAM);
        if !one.name.is_empty() {
            die.attrs.push((AT_NAME, Value::Str(one.name.clone())));
        }
        if self.object.symbols[one.symbol].binding == Binding::Public {
            die.attrs.push((AT_EXTERNAL, Value::Flag));
        }
        die.attrs.push((AT_PROTOTYPED, Value::Flag));
        if let Some(Type::Procedure { result: Some(result), .. }) = self.info.types.get(one.r#type) {
            self.typed(&mut die, *result)?;
        }
        die.attrs.push((AT_LOW_PC, self.address(range.section, range.offset)?));
        die.attrs.push((AT_HIGH_PC, Value::Len(range.length as u32)));
        let framed = one.variables.iter().chain(one.blocks.iter().flat_map(|block| &block.variables)).any(|variable| matches!(variable.location, Location::Frame { .. }));
        if framed {
            let base = self.info.registers.iter().find(|register| register.name == self.info.frame_register).map(|register| register.name.clone()).ok_or_else(|| Unsupported("DWARF: no frame register".into()))?;
            die.attrs.push((AT_FRAME_BASE, Value::Expr(self.register(&base)?)));
        }
        self.scope(&mut die, &one.variables, &one.blocks)?;
        Ok(self.push(die))
    }
}

fn base(scalar: Scalar) -> Result<Die, Unsupported> {
    let (name, encoding, bytes): (String, u8, u8) = match scalar {
        Scalar::Void => unreachable!("void has no DIE"),
        Scalar::Bool { bytes } => ("bool".into(), ATE_BOOLEAN, bytes),
        Scalar::Char => ("char".into(), ATE_SIGNED_CHAR, 1),
        Scalar::Int { bytes, signed } => (format!("{}int{}_t", if signed { "" } else { "u" }, u32::from(bytes) * 8), if signed { ATE_SIGNED } else { ATE_UNSIGNED }, bytes),
        Scalar::Float { bytes } => (match bytes { 4 => "float", 8 => "double", _ => "long double" }.into(), ATE_FLOAT, bytes),
        Scalar::Currency => return refused("BASIC's CURRENCY has no DWARF type"),
        Scalar::BasicString { .. } => return refused("BASIC's STRING has no DWARF type"),
    };
    let mut die = Die::new(TAG_BASE);
    die.attrs.push((AT_NAME, Value::Str(name)));
    die.attrs.push((AT_ENCODING, Value::U8(encoding)));
    die.attrs.push((AT_BYTE_SIZE, Value::U8(bytes)));
    Ok(die)
}

/// The tree: the compile unit is DIE 0 and type `i` is DIE `1 + i`.
pub fn tree(object: &Object, info: &Info) -> Result<Vec<Die>, Unsupported> {
    let mut tree = Tree { object, info, dies: Vec::new(), bad: vec![None; info.types.len()] };
    let mut unit = Die::new(TAG_COMPILE_UNIT);
    unit.attrs.push((AT_PRODUCER, Value::Str("llrm".into())));
    unit.attrs.push((AT_LANGUAGE, Value::U16(language(info.language))));
    unit.attrs.push((AT_NAME, Value::Str(info.files.first().map_or(String::new(), |one| one.name.clone()))));
    match info.code[..] {
        [] => {}
        [range] => {
            unit.attrs.push((AT_LOW_PC, tree.address(range.section, range.offset)?));
            unit.attrs.push((AT_HIGH_PC, Value::Len(range.length as u32)));
        }
        _ => return refused("a module of several code ranges is not written yet"),
    }
    unit.attrs.push((AT_STMT_LIST, Value::Line));
    tree.dies.push(unit);
    tree.dies.extend((0..info.types.len()).map(|_| Die::new(0)));
    let mut children = Vec::new();
    for index in 0..info.types.len() {
        match tree.describe(index) {
            Ok(()) => {
                if tree.ty(index)?.is_some() {
                    children.push(1 + index);
                }
            }
            Err(Unsupported(why)) => tree.bad[index] = Some(why.trim_start_matches("DWARF: ").to_owned()),
        }
    }
    for global in &info.globals {
        let at = tree.variable(global, true)?;
        children.push(at);
    }
    for function in &info.functions {
        let at = tree.function(function)?;
        children.push(at);
    }
    tree.dies[0].children = children;
    Ok(tree.dies)
}
