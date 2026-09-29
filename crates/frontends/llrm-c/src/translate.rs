//! One unit's trees as llrm's HIR: each C object a place or a data object,
//! each tree node its instructions. `hir::mir` makes the MIR.
//!
//! What C promises is marked in the HIR: its aliasing classes, `restrict`,
//! internal linkage for what the unit does not export, `nowrap` on signed
//! arithmetic, `inbounds` on pointer arithmetic. The last two hold for
//! objects of at most PTRDIFF_MAX bytes, as clang assumes of every object.

use std::collections::{HashMap, HashSet};

use llrm_core::hir::model::{
    self as h, AddressKind, CallDistance, DataLinkage, FloatEvaluation, FloatReturn, Number, Op, Operand, StackCleanup, Storage, TerminatorKind, TypeKind,
};
use llrm_core::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::hir::{self, Unsupported};
use crate::raise_hir::{EMITTED, classes, far_pointers, is_float, library_routine, pointers, signed, widths};

type R<T> = Result<T, Unsupported>;

fn refuse<T>(what: impl Into<String>) -> R<T> {
    Err(Unsupported(what.into()))
}

/// The unit as a HIR program of one module.
pub fn program(unit: &hir::Unit, name: &str) -> R<h::Program> {
    let mut types = Types::new(unit);
    let objects = objects(unit)?;
    let mut keys: HashMap<Key, i64> = HashMap::new();
    for (at, object) in objects.iter().enumerate() {
        keys.insert(object.key, at as i64 + 1);
    }
    let imports: Vec<&hir::Symbol> = unit.symbols.values().filter(|one| !one.proc() && one.imported()).collect();
    for symbol in &imports {
        keys.insert(Key::Symbol(symbol.id), (keys.len() + 1) as i64);
    }
    let mut callables: IndexMap<String, h::Callable> = IndexMap::default();
    let mut data = Vec::new();
    for object in &objects {
        data.push(data_object(unit, object, keys[&object.key], &keys, &mut callables)?);
    }
    for symbol in imports {
        let id = keys[&Key::Symbol(symbol.id)];
        data.push(h::DataObject { linkage: DataLinkage::External, address: address(space(unit, Key::Symbol(symbol.id))), ..h::DataObject::new(id, &symbol.object_name(), Vec::new()) });
    }
    let valueless: HashSet<i64> = unit
        .procs
        .iter()
        .filter(|proc| proc.body.iter().filter(|one| one.call == "CGReturn").all(|one| one.args[0] == "n0") && !answered_by_inline_code(unit, proc))
        .map(|proc| proc.symbol)
        .collect();
    let module = Shared { unit, data: &data, keys: &keys, valueless: &valueless };
    let mut functions = Vec::new();
    for (at, proc) in unit.procs.iter().enumerate() {
        functions.push(Body::function(&module, &mut types, &mut callables, proc, at as i64 + 1)?);
    }
    let defined: HashSet<&str> = functions.iter().map(|one: &h::Function| one.name.as_str()).collect();
    let callables: Vec<h::Callable> = callables.into_values().filter(|one| one.defined || !defined.contains(one.name.as_str())).collect();
    let promises = h::RuntimePromises { reads_arguments: crate::libfunc::reads_arguments(callables.iter().map(|one| one.name.as_str())), ..Default::default() };
    let (types, alias_classes) = types.finished();
    let module = h::Module { data, callables, alias_classes, ..h::Module::new(1, name, types, functions) };
    // Borland's medium model: a call keeps what its contract does not clobber;
    // the compiler's constants go in CONST.
    let preserved = llrm_core::abi::runtime::preserves(&crate::raise_hir::medium_model(String::new(), true, 0));
    Ok(h::Program {
        zeroed_locals: false,
        promises,
        preserved: preserved.iter().map(|one| one.value().to_owned()).collect(),
        constant_segment: Some("CONST".to_owned()),
        ..h::Program::new(h::Dialect::C, h::RuntimeProfile::Freestanding, vec![module])
    })
}

// ---- data ----

/// What a data label names: a symbol, or a literal the front end placed.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Key {
    Symbol(i64),
    Literal(i64),
}

fn back_key(unit: &hir::Unit, back: i64) -> Key {
    match unit.backs.get(&back).copied().unwrap_or(0) {
        0 => Key::Literal(back),
        symbol => Key::Symbol(symbol),
    }
}

/// A far pointer's address space, and far code's.
const FAR: u32 = 1;

/// Where a symbol or literal is: far unless in DGROUP.
fn space(unit: &hir::Unit, key: Key) -> u32 {
    match key {
        Key::Symbol(symbol) => {
            let symbol = &unit.symbols[&symbol];
            match (symbol.proc(), symbol.far(), unit.grouped(symbol)) {
                (true, true, _) | (false, _, false) => FAR,
                _ => 0,
            }
        }
        Key::Literal(_) => 0,
    }
}

fn address(space: u32) -> AddressKind {
    if space == FAR { AddressKind::Far } else { AddressKind::Near }
}

/// An address in data: at `at`, `far` or near, to `offset` bytes into `target`.
struct Relocation {
    at: usize,
    target: Key,
    offset: i64,
    far: bool,
}

/// A labelled data object: its bytes, the addresses in them, and where it goes.
struct Object {
    key: Key,
    name: String,
    segment: String,
    bytes: Vec<u8>,
    relocations: Vec<Relocation>,
    align: Option<u64>,
}

fn number(text: &str) -> R<i64> {
    text.trim().parse().map_err(|_| Unsupported(format!("not a number: {text}")))
}

/// Each data segment's labelled objects, as the stream lays them down.
fn objects(unit: &hir::Unit) -> R<Vec<Object>> {
    let mut out: Vec<Object> = Vec::new();
    for segment in unit.segments.values().filter(|one| one.attr & 0x1 == 0) {
        let first = out.len();
        let mut align = None;
        for (call, args) in &segment.items {
            let args: Vec<&str> = args.0.iter().map(String::as_str).collect();
            if let ("DGLabel", [back]) = (call.as_str(), &args[..]) {
                let key = back_key(unit, hir::handle(back));
                let name = match key {
                    Key::Symbol(symbol) => unit.symbols[&symbol].object_name(),
                    Key::Literal(back) => format!("L_b{back}"),
                };
                out.push(Object { key, name, segment: segment.name.clone(), bytes: Vec::new(), relocations: Vec::new(), align: align.take() });
                continue;
            }
            if let ("DGAlign", [to]) = (call.as_str(), &args[..]) {
                align = Some(number(to)? as u64);
                continue;
            }
            let Some(object) = out[first..].last_mut() else { return refuse(format!("{} data before any label", segment.name)) };
            let pointer = |object: &mut Object, target: Key, offset: &str, far: bool| -> R<()> {
                object.relocations.push(Relocation { at: object.bytes.len(), target, offset: number(offset)?, far });
                object.bytes.extend(std::iter::repeat_n(0, if far { 4 } else { 2 }));
                Ok(())
            };
            match (call.as_str(), &args[..]) {
                ("DGUBytes", [size]) => object.bytes.extend(std::iter::repeat_n(0, number(size)? as usize)),
                ("DGIBytes", [size, byte]) => object.bytes.extend(std::iter::repeat_n(number(byte)? as u8, number(size)? as usize)),
                ("DGBytes", [_, data]) => object.bytes.extend(crate::compile::hex_bytes(data)),
                ("DGInteger", [value, type_]) => {
                    let width = widths(type_).unwrap_or(2) as usize;
                    object.bytes.extend(&number(value)?.to_le_bytes()[..width]);
                }
                ("DGFEPtr", [symbol, type_, offset]) => {
                    let far = far_pointers(type_) || *type_ == "TY_LONG_CODE_PTR" || (*type_ == "TY_CODE_PTR" && unit.target & hir::BIG_CODE != 0);
                    pointer(object, Key::Symbol(hir::handle(symbol)), offset, far)?;
                }
                ("DGBackPtr", [back, _, offset, type_]) => pointer(object, back_key(unit, hir::handle(back)), offset, far_pointers(type_))?,
                _ => return refuse(format!("data item {call} {}", args.join(" "))),
            }
        }
    }
    Ok(out)
}

fn data_object(unit: &hir::Unit, object: &Object, id: i64, keys: &HashMap<Key, i64>, callables: &mut IndexMap<String, h::Callable>) -> R<h::DataObject> {
    let mut relocations = Vec::new();
    for relocation in &object.relocations {
        let address = if relocation.far { AddressKind::Far } else { AddressKind::Near };
        if let Key::Symbol(symbol) = relocation.target
            && unit.symbols[&symbol].proc()
        {
            let symbol = &unit.symbols[&symbol];
            if inline(symbol) {
                return refuse(format!("{}: an address of inline code {}", object.name, symbol.name));
            }
            let defined = unit.procs.iter().any(|one| one.symbol == symbol.id);
            let target = callable(callables, &symbol.object_name(), defined);
            relocations.push(h::DataRelocation { at: relocation.at as i64, target, addend: relocation.offset, address, code: true });
            continue;
        }
        let Some(&target) = keys.get(&relocation.target) else {
            return refuse(format!("{}: an address of {:?}, which the unit neither defines nor imports", object.name, relocation.target));
        };
        relocations.push(h::DataRelocation { at: relocation.at as i64, target, addend: relocation.offset, address, code: false });
    }
    let (linkage, readonly) = match object.key {
        Key::Symbol(symbol) => {
            let symbol = &unit.symbols[&symbol];
            (if symbol.exported() { DataLinkage::Exported } else { DataLinkage::Internal }, symbol.constant())
        }
        Key::Literal(_) => (DataLinkage::Private, true),
    };
    Ok(h::DataObject {
        readonly,
        relocations,
        linkage,
        address: address(space(unit, object.key)),
        segment: Some(object.segment.clone()),
        align: object.align.map(|one| one as i64),
        ..h::DataObject::new(id, &object.name, object.bytes.iter().map(|&one| i64::from(one)).collect())
    })
}

// ---- types ----

/// What a HIR type holds, as MIR sees it.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum Shape {
    Void,
    Int(i64, bool),
    Bool,
    Float(i64),
    Pointer(i64),
    Bytes(i64),
}

/// C's aliasing classes as clang's `!tbaa` tree: every class under the
/// character type, which may alias anything.
const ROOT: &str = "Simple C/C++ TBAA";
const CHAR: &str = "omnipotent char";
const CLASSES: [&str; 7] = ["int2", "int4", "int8", "float4", "float8", "pointer2", "pointer4"];

/// The module's types, each made once, by shape and aliasing class.
struct Types<'u> {
    unit: &'u hir::Unit,
    list: Vec<h::Type>,
    made: HashMap<(Shape, Option<&'static str>), i64>,
    members: HashMap<&'static str, Vec<i64>>,
}

impl<'u> Types<'u> {
    fn new(unit: &'u hir::Unit) -> Self {
        Types { unit, list: Vec::new(), made: HashMap::new(), members: HashMap::new() }
    }

    fn get(&self, id: i64) -> &h::Type {
        &self.list[id as usize - 1]
    }

    fn shape(&self, id: i64) -> Shape {
        let one = self.get(id);
        match one.kind {
            TypeKind::Void => Shape::Void,
            TypeKind::Boolean => Shape::Bool,
            TypeKind::Integer => Shape::Int(one.width, one.signed == Some(true)),
            TypeKind::Float => Shape::Float(one.width),
            TypeKind::Pointer => Shape::Pointer(one.width),
            TypeKind::Array | TypeKind::Opaque => Shape::Bytes(one.width),
        }
    }

    fn of(&mut self, shape: Shape, class: Option<&'static str>) -> i64 {
        if let Some(&id) = self.made.get(&(shape, class)) {
            return id;
        }
        let id = self.list.len() as i64 + 1;
        let (kind, width) = match shape {
            Shape::Void => (TypeKind::Void, 0),
            Shape::Int(width, _) => (TypeKind::Integer, width),
            Shape::Bool => (TypeKind::Boolean, 2),
            Shape::Float(width) => (TypeKind::Float, width),
            Shape::Pointer(width) => (TypeKind::Pointer, width),
            Shape::Bytes(width) => (TypeKind::Opaque, width),
        };
        let mut made = h::Type::new(id, &format!("{shape:?}{}", class.map_or(String::new(), |one| format!(" {one}"))), kind, width);
        match shape {
            Shape::Int(_, is_signed) => made.signed = Some(is_signed),
            Shape::Bool => made.signed = Some(false),
            Shape::Float(4) => made.evaluation = FloatEvaluation::Binary32,
            Shape::Float(_) => made.evaluation = FloatEvaluation::Binary64,
            Shape::Pointer(width) => made.address = address(if width == 4 { FAR } else { 0 }),
            _ => {}
        }
        self.list.push(made);
        self.made.insert((shape, class), id);
        if let Some(class) = class {
            self.members.entry(class).or_default().push(id);
        }
        id
    }

    fn int(&mut self, bytes: i64, is_signed: bool) -> i64 {
        self.of(Shape::Int(bytes, is_signed), None)
    }

    fn pointer(&mut self, space: u32) -> i64 {
        self.of(Shape::Pointer(if space == FAR { 4 } else { 2 }), None)
    }

    /// An aggregate's size; none for a scalar.
    fn aggregate(&self, type_: &str) -> Option<i64> {
        self.unit.types.get(&self.unit.canonical_type(type_)).copied()
    }

    /// C type `type_` as a value, its accesses in its aliasing class.
    fn c(&mut self, type_: &str) -> R<i64> {
        let type_ = self.unit.canonical_type(type_);
        let big = |flag| if self.unit.target & flag != 0 { 4 } else { 2 };
        let shape = match type_.as_str() {
            "TY_POINTER" => Shape::Pointer(big(hir::BIG_DATA)),
            "TY_CODE_PTR" => Shape::Pointer(big(hir::BIG_CODE)),
            "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => Shape::Pointer(2),
            "TY_LONG_POINTER" | "TY_HUGE_POINTER" | "TY_LONG_CODE_PTR" => Shape::Pointer(4),
            "TY_SINGLE" => Shape::Float(4),
            "TY_DOUBLE" => Shape::Float(8),
            other => match (widths(other), self.aggregate(other)) {
                (Some(width), _) => Shape::Int(i64::from(width), signed(other)),
                (None, Some(size)) => return refuse(format!("a {size}-byte aggregate as a value")),
                (None, None) => return refuse(format!("no MIR type for {other}")),
            },
        };
        let class = match type_.as_str() {
            "TY_POINTER" => Some(if self.unit.target & hir::BIG_DATA != 0 { "pointer4" } else { "pointer2" }),
            other => classes(other),
        };
        Ok(self.of(shape, Some(class.unwrap_or(CHAR))))
    }

    /// Raw memory `bytes` wide, as a copy moves it: the character type's class.
    fn raw(&mut self, bytes: i64) -> i64 {
        self.of(Shape::Int(bytes, false), Some(CHAR))
    }

    fn finished(self) -> (Vec<h::Type>, Vec<h::AliasClass>) {
        let mut classes = vec![h::AliasClass { name: ROOT.to_owned(), parent: None, types: Vec::new() }];
        let mut members = self.members;
        for (name, parent) in std::iter::once((CHAR, ROOT)).chain(CLASSES.iter().map(|&one| (one, CHAR))) {
            let types = members.remove(name).unwrap_or_default();
            classes.push(h::AliasClass { name: name.to_owned(), parent: Some(parent.to_owned()), types });
        }
        (self.list, classes)
    }
}

// ---- bodies ----

/// What the functions share.
struct Shared<'a> {
    unit: &'a hir::Unit,
    data: &'a [h::DataObject],
    keys: &'a HashMap<Key, i64>,
    /// Each defined procedure no return gives a value: a void function,
    /// which the stream types as an int.
    valueless: &'a HashSet<i64>,
}

/// What a tree node evaluates to.
#[derive(Clone, Copy, Debug)]
enum Got {
    /// A scalar, or an lvalue's address.
    Value(i64),
    /// An lvalue's address, accessed volatile.
    Volatile(i64),
    /// `size` bytes of aggregate at an address.
    Aggregate(i64, i64),
    /// A function designator, by its symbol.
    Function(i64),
    /// A call's result, before O_POINTS reads it; none from a void call.
    Returned(Option<i64>),
}

/// A block being built.
struct Open {
    id: i64,
    instructions: Vec<h::Instruction>,
    terminator: Option<h::Terminator>,
}

struct Body<'a, 't> {
    shared: &'a Shared<'a>,
    unit: &'a hir::Unit,
    proc: &'a hir::Proc,
    types: &'t mut Types<'a>,
    callables: &'t mut IndexMap<String, h::Callable>,
    values: Vec<h::Value>,
    places: Vec<h::Place>,
    frame: i64,
    blocks: Vec<Open>,
    current: usize,
    labels: HashMap<String, usize>,
    slots: HashMap<String, i64>,
    globals: HashMap<i64, i64>,
    done: HashMap<i64, Got>,
    selects: IndexMap<String, (Vec<(i64, String)>, Option<String>)>,
    calls: Vec<h::CallAbi>,
    instructions: i64,
    /// What the last statement's inline code left in dx:ax.
    inlined: Option<i64>,
}

/// Whether inline code is `symbol`: `_asm`, or `__emit__`'s bytes.
fn inline(symbol: &hir::Symbol) -> bool {
    symbol.code.is_some() || EMITTED.contains(&symbol.name.as_str())
}

/// The inline code a statement runs as its last call, if it is a
/// `CGDone` of one.
fn inline_statement(unit: &hir::Unit, one: &hir::Statement) -> bool {
    let (Some(node), "CGDone") = (one.args.first(), one.call.as_str()) else { return false };
    let Some(tree) = unit.nodes.get(&hir::handle(node)) else { return false };
    let Some(call) = tree.args.first().filter(|_| tree.call == "CGCall").and_then(|call| unit.calls.get(&hir::handle(call))) else { return false };
    unit.nodes.get(&hir::handle(&call.target)).is_some_and(|target| {
        target.call == "CGFEName" && target.args.first().and_then(|symbol| unit.symbols.get(&hir::handle(symbol))).is_some_and(inline)
    })
}

/// Whether a value-less return of `proc` follows inline code: Borland's
/// convention returns what that code left in dx:ax.
fn answered_by_inline_code(unit: &hir::Unit, proc: &hir::Proc) -> bool {
    let statements: Vec<&hir::Statement> = proc.body.iter().filter(|one| one.call != "DBSrcCue").collect();
    statements.windows(2).any(|two| two[1].call == "CGReturn" && two[1].args[0] == "n0" && inline_statement(unit, two[0]))
}

fn value_ref(value: i64) -> Operand {
    Operand::value_ref(value)
}

/// The stack convention of a procedure: cdecl's, or pascal's, which
/// pushes in order and pops its own.
fn cleanup(symbol: &hir::Symbol) -> R<StackCleanup> {
    let stack = symbol.call_class & (hir::CALLER_POPS | hir::REVERSE_PARMS);
    match (symbol.register_parms, stack) {
        (false, hir::CALLER_POPS) => Ok(StackCleanup::Caller),
        (false, hir::REVERSE_PARMS) => Ok(StackCleanup::Callee),
        _ => refuse(format!("{} has a register calling convention", symbol.object_name())),
    }
}

fn distance(symbol: &hir::Symbol) -> CallDistance {
    if symbol.far() { CallDistance::Far } else { CallDistance::Near }
}

/// Whether some restrict lvalue names parameter `symbol`.
fn restricted(unit: &hir::Unit, symbol: i64) -> bool {
    unit.nodes.values().any(|node| match (node.call.as_str(), &node.args[..]) {
        ("CGAttr", [inner, attr]) if attr == "3" => {
            let inner = &unit.nodes[&hir::handle(inner)];
            inner.call == "CGFEName" && hir::handle(&inner.args[0]) == symbol
        }
        _ => false,
    })
}

impl<'a, 't> Body<'a, 't> {
    fn function(shared: &'a Shared<'a>, types: &'t mut Types<'a>, callables: &'t mut IndexMap<String, h::Callable>, proc: &'a hir::Proc, id: i64) -> R<h::Function> {
        let unit = shared.unit;
        let symbol = &unit.symbols[&proc.symbol];
        let cleanup = cleanup(symbol)?;
        let mut body = Body {
            shared,
            unit,
            proc,
            types,
            callables,
            values: Vec::new(),
            places: Vec::new(),
            frame: 0,
            blocks: Vec::new(),
            current: 0,
            labels: HashMap::new(),
            slots: HashMap::new(),
            globals: HashMap::new(),
            done: HashMap::new(),
            selects: IndexMap::default(),
            calls: Vec::new(),
            instructions: 0,
            inlined: None,
        };
        let entry = body.block();
        body.current = entry;
        let result_type = if shared.valueless.contains(&symbol.id) { body.types.of(Shape::Void, None) } else { body.ty(&proc.type_)? };
        let (parameters, promises) = body.frame(cleanup)?;
        for one in &proc.body {
            body.statement(one)?;
        }
        body.finish()?;
        let parameter_bytes = parameters.iter().map(|&one| body.types.get(body.values[one as usize - 1].r#type).width).sum();
        let blocks = body
            .blocks
            .into_iter()
            .map(|one| h::Block::new(one.id, one.instructions, one.terminator.expect("finished")))
            .collect();
        let linkage = if symbol.exported() { h::FunctionLinkage::External } else { h::FunctionLinkage::Internal };
        Ok(h::Function {
            parameters,
            abi: Some(h::ProcedureAbi { cleanup, distance: distance(symbol), parameter_bytes, float_return: FloatReturn::Register }),
            calls: body.calls,
            linkage,
            promises,
            ..h::Function::new(id, &symbol.object_name(), result_type, body.values, body.places, blocks, 1)
        })
    }

    fn name(&self) -> String {
        self.unit.symbols[&self.proc.symbol].object_name()
    }

    fn refuse<T>(&self, what: impl std::fmt::Display) -> R<T> {
        refuse(format!("{}: {what}", self.name()))
    }

    fn ty(&mut self, type_: &str) -> R<i64> {
        let result = self.types.c(type_);
        result.map_err(|error| Unsupported(format!("{}: {}", self.name(), error.0)))
    }

    // ---- values ----

    fn value(&mut self, ty: i64) -> i64 {
        let id = self.values.len() as i64 + 1;
        self.values.push(h::Value { id, r#type: ty });
        id
    }

    fn type_of(&self, value: i64) -> i64 {
        self.values[value as usize - 1].r#type
    }

    fn shape(&self, value: i64) -> Shape {
        self.types.shape(self.type_of(value))
    }

    /// An integer's bits.
    fn bits(&self, value: i64) -> Option<i64> {
        match self.shape(value) {
            Shape::Int(width, _) => Some(width * 8),
            Shape::Bool => Some(16),
            _ => None,
        }
    }

    /// A pointer's address space.
    fn space(&self, value: i64) -> Option<u32> {
        match self.shape(value) {
            Shape::Pointer(4) => Some(FAR),
            Shape::Pointer(_) => Some(0),
            _ => None,
        }
    }

    /// Whether two types are one MIR type.
    fn same(&self, a: i64, b: i64) -> bool {
        let mir = |shape| match shape {
            Shape::Int(width, _) => Shape::Int(width, false),
            Shape::Bool => Shape::Int(2, false),
            other => other,
        };
        mir(self.types.shape(a)) == mir(self.types.shape(b))
    }

    fn instruction(&mut self, op: Op, results: Vec<i64>, operands: Vec<Operand>) -> &mut h::Instruction {
        self.instructions += 1;
        let made = h::Instruction::new(self.instructions, op, results, operands);
        let block = &mut self.blocks[self.current];
        block.instructions.push(made);
        block.instructions.last_mut().expect("just pushed")
    }

    /// `op` of `operands`, a value of type `ty`.
    fn op(&mut self, op: Op, ty: i64, operands: Vec<Operand>) -> i64 {
        let result = self.value(ty);
        self.instruction(op, vec![result], operands);
        result
    }

    fn constant(&mut self, ty: i64, value: Number) -> i64 {
        self.op(Op::Copy, ty, vec![Operand::Constant(h::Constant { r#type: ty, value })])
    }

    fn int(&mut self, bytes: i64, value: i64) -> i64 {
        let ty = self.types.int(bytes, true);
        self.constant(ty, Number::Int(value))
    }

    /// `value` retyped as an integer of its width and `signedness`.
    fn as_signed(&mut self, value: i64, is_signed: bool) -> i64 {
        match self.shape(value) {
            Shape::Int(_, now) if now == is_signed => value,
            Shape::Int(width, _) => {
                let ty = self.types.int(width, is_signed);
                self.op(Op::Copy, ty, vec![value_ref(value)])
            }
            Shape::Bool => {
                let ty = self.types.int(2, is_signed);
                self.op(Op::Copy, ty, vec![value_ref(value)])
            }
            _ => value,
        }
    }

    // ---- blocks ----

    fn block(&mut self) -> usize {
        let id = self.blocks.len() as i64 + 1;
        self.blocks.push(Open { id, instructions: Vec::new(), terminator: None });
        self.blocks.len() - 1
    }

    fn terminated(&self, block: usize) -> bool {
        self.blocks[block].terminator.is_some()
    }

    fn terminate(&mut self, kind: TerminatorKind, operands: Vec<Operand>, targets: Vec<usize>) -> &mut h::Terminator {
        let targets = targets.into_iter().map(|one| self.blocks[one].id).collect();
        let block = &mut self.blocks[self.current];
        block.terminator.insert(h::Terminator::new(kind, operands, targets))
    }

    fn jump(&mut self, target: usize) {
        self.terminate(TerminatorKind::Jump, Vec::new(), vec![target]);
    }

    fn position(&mut self, block: usize) {
        self.current = block;
    }

    fn label(&mut self, name: &str) -> usize {
        if let Some(&block) = self.labels.get(name) {
            return block;
        }
        let block = self.block();
        self.labels.insert(name.to_owned(), block);
        block
    }

    /// Code from here on goes into `block`, which the code before falls into.
    fn place(&mut self, block: usize) {
        if !self.terminated(self.current) {
            self.jump(block);
        }
        self.position(block);
    }

    /// A block for code after a terminator, which nothing reaches unless a label follows.
    fn open(&mut self) {
        if self.terminated(self.current) {
            let block = self.block();
            self.position(block);
        }
    }

    fn predecessors(&self, block: usize) -> bool {
        let id = self.blocks[block].id;
        self.blocks.iter().filter_map(|one| one.terminator.as_ref()).any(|one| one.targets.contains(&id) || one.cases.iter().any(|&(_, target)| target == id))
    }

    fn finish(&mut self) -> R<()> {
        if !self.terminated(self.current) {
            if self.current == 0 || self.predecessors(self.current) {
                return self.refuse("control reaches the end with no return");
            }
            self.terminate(TerminatorKind::Unreachable, Vec::new(), Vec::new());
        }
        if self.labels.values().any(|&one| !self.terminated(one)) {
            return self.refuse("a label no statement places");
        }
        Ok(())
    }

    // ---- places ----

    /// A local place of type `ty`, `size` bytes, after the frame's others.
    fn local(&mut self, name: &str, ty: i64, size: i64) -> i64 {
        let id = self.places.len() as i64 + 1;
        let place = h::Place { extent: Some(size), ..h::Place::new(id, name, ty, Storage::Local, self.frame) };
        self.places.push(place);
        self.frame += size.max(1);
        id
    }

    /// A place's address.
    fn address_of(&mut self, place: i64) -> i64 {
        let space = if self.places[place as usize - 1].address == AddressKind::Far { FAR } else { 0 };
        let ty = self.types.pointer(space);
        self.op(Op::Address, ty, vec![Operand::place_ref(place)])
    }

    /// The place over data object `object`, as far as the unit lays it out.
    fn global(&mut self, object: i64) -> i64 {
        if let Some(&place) = self.globals.get(&object) {
            return place;
        }
        let data = &self.shared.data[object as usize - 1];
        let size = data.bytes.len() as i64;
        let ty = self.types.of(Shape::Bytes(size), None);
        let storage = if data.linkage == DataLinkage::External { Storage::External } else { Storage::Module };
        let id = self.places.len() as i64 + 1;
        let place = h::Place { symbol: object, extent: Some(size), address: data.address, ..h::Place::new(id, &data.name, ty, storage, 0) };
        self.places.push(place);
        self.globals.insert(object, id);
        id
    }

    /// Each parameter and auto a place; each parameter stored into its own.
    fn frame(&mut self, cleanup: StackCleanup) -> R<(Vec<i64>, Vec<h::Promise>)> {
        let mut parameters = Vec::new();
        let mut promises = Vec::new();
        let mut stores = Vec::new();
        for (symbol, type_) in self.proc.parameters(&self.unit.symbols[&self.proc.symbol]) {
            match self.types.aggregate(type_) {
                Some(_) if cleanup != StackCleanup::Caller => return self.refuse("an aggregate argument by a callee-pops convention"),
                Some(size) => {
                    let words = (size + 1) / 2;
                    let ty = self.types.of(Shape::Bytes(words * 2), None);
                    let place = self.local(&format!("y{symbol}"), ty, words * 2);
                    let word = self.types.raw(2);
                    let first = parameters.len();
                    for at in 0..words {
                        parameters.push(self.value(word));
                        stores.push((place, at * 2, parameters[first + at as usize]));
                    }
                    self.slots.insert(format!("y{symbol}"), place);
                }
                None => {
                    let ty = self.ty(type_)?;
                    let size = self.types.get(ty).width;
                    let place = self.local(&format!("y{symbol}"), ty, size);
                    let parameter = self.value(ty);
                    parameters.push(parameter);
                    stores.push((place, -1, parameter));
                    self.slots.insert(format!("y{symbol}"), place);
                    // C99 6.7.3.1: what a restrict parameter reaches, nothing else in its block does.
                    if restricted(self.unit, *symbol) {
                        promises.push(h::Promise { parameter, bytes: 0, unaliased: true, readonly: false });
                    }
                }
            }
        }
        for (key, type_) in &self.proc.autos {
            let (ty, size) = match self.types.aggregate(type_) {
                Some(size) => (self.types.of(Shape::Bytes(size), None), size),
                None => {
                    let ty = self.ty(type_)?;
                    (ty, self.types.get(ty).width)
                }
            };
            let place = self.local(key, ty, size);
            self.slots.insert(key.clone(), place);
        }
        for (place, at, parameter) in stores {
            let target = if at < 0 {
                Operand::place_ref(place)
            } else {
                let base = self.address_of(place);
                let ty = self.type_of(parameter);
                Operand::IndirectPlace(indirect(base, at, ty, false))
            };
            self.instruction(Op::Store, Vec::new(), vec![target, value_ref(parameter)]);
        }
        Ok((parameters, promises))
    }

    // ---- statements ----

    fn statement(&mut self, one: &hir::Statement) -> R<()> {
        let args: Vec<&str> = one.args.iter().map(String::as_str).collect();
        if !matches!(one.call.as_str(), "DBSrcCue" | "CGReturn") {
            self.inlined = None;
        }
        match (one.call.as_str(), &args[..]) {
            ("CGControl", ["O_LABEL", _, label]) => {
                let block = self.label(label);
                self.place(block);
                return Ok(());
            }
            _ => self.open(),
        }
        match (one.call.as_str(), &args[..]) {
            ("CGDone" | "CGTrash", [node]) => {
                self.eval(node)?;
            }
            ("CGControl", ["O_GOTO", _, label]) => {
                let block = self.label(label);
                self.jump(block);
            }
            ("CGControl", [test @ ("O_IF_TRUE" | "O_IF_FALSE"), node, label]) => {
                let block = self.label(label);
                self.branch(node, block, *test == "O_IF_TRUE")?;
            }
            ("CGReturn", [node, type_]) => self.ret(node, type_)?,
            ("CGSelInit", [select]) => {
                self.selects.insert((*select).to_owned(), (Vec::new(), None));
            }
            ("CGSelCase", [select, label, value]) => {
                let Some((cases, _)) = self.selects.get_mut(*select) else { return self.refuse(format!("case of no select {select}")) };
                cases.push((hir::int(value), (*label).to_owned()));
            }
            ("CGSelOther", [select, label]) => {
                let Some((_, other)) = self.selects.get_mut(*select) else { return self.refuse(format!("default of no select {select}")) };
                *other = Some((*label).to_owned());
            }
            ("CGSelect", [select, node]) => {
                let Some((cases, Some(other))) = self.selects.shift_remove(*select) else { return self.refuse(format!("select {select} with no default")) };
                let from = self.type_of_node(node);
                let got = self.eval(node)?;
                let value = self.scalar(got)?;
                let bits = self.bits(value).ok_or_else(|| Unsupported(format!("{}: a switch on a non-integer", self.name())))?.max(16);
                let is_signed = signed(&self.unit.canonical_type(&from));
                let wide = self.types.int(bits / 8, is_signed);
                let value = self.resized(value, is_signed, wide);
                let default = self.label(&other);
                let arms: Vec<(i64, usize)> = cases.iter().map(|(case, label)| (*case, self.label(label))).collect();
                let arms = arms.into_iter().map(|(case, block)| (case, self.blocks[block].id)).collect();
                self.terminate(TerminatorKind::Switch, vec![value_ref(value)], vec![default]).cases = arms;
            }
            _ => return refuse(format!("{} line {}: {} {}", self.name(), one.line, one.call, one.args.join(" "))),
        }
        Ok(())
    }

    fn ret(&mut self, node: &str, type_: &str) -> R<()> {
        if self.shared.valueless.contains(&self.proc.symbol) {
            if node != "n0" {
                self.eval(node)?;
            }
            self.terminate(TerminatorKind::Return, Vec::new(), Vec::new());
            return Ok(());
        }
        if let (Some(answer), "n0") = (self.inlined, node) {
            let value = self.converted(answer, "TY_UINT_4", &self.proc.type_.clone())?;
            self.terminate(TerminatorKind::Return, vec![value_ref(value)], Vec::new());
            return Ok(());
        }
        if node == "n0" {
            // A value-less return from a function that has one: nothing the caller may read.
            self.terminate(TerminatorKind::Return, Vec::new(), Vec::new());
            return Ok(());
        }
        let value = self.value_as(node, type_)?;
        let value = self.converted(value, type_, &self.proc.type_.clone())?;
        self.terminate(TerminatorKind::Return, vec![value_ref(value)], Vec::new());
        Ok(())
    }

    /// Go to `target` when `node` is `when`; fall through otherwise.
    fn branch(&mut self, node: &str, target: usize, when: bool) -> R<()> {
        let tree = &self.unit.nodes[&hir::handle(node)];
        let args: Vec<&str> = tree.args.iter().map(String::as_str).collect();
        match (tree.call.as_str(), &args[..]) {
            ("CGFlow", ["O_FLOW_NOT", inner, _]) => return self.branch(inner, target, !when),
            ("CGFlow", [flow @ ("O_FLOW_AND" | "O_FLOW_OR"), left, right]) => {
                if (*flow == "O_FLOW_OR") == when {
                    self.branch(left, target, when)?;
                    return self.branch(right, target, when);
                }
                let skip = self.block();
                self.branch(left, skip, !when)?;
                self.branch(right, target, when)?;
                self.place(skip);
                return Ok(());
            }
            _ => {}
        }
        let condition = match tree.call.as_str() {
            "CGCompare" => self.compare(&tree.args)?,
            _ => {
                let got = self.eval(node)?;
                let value = self.scalar(got)?;
                self.nonzero(value)
            }
        };
        let fall = self.block();
        let (taken, otherwise) = if when { (target, fall) } else { (fall, target) };
        self.terminate(TerminatorKind::Branch, vec![value_ref(condition)], vec![taken, otherwise]);
        self.position(fall);
        Ok(())
    }

    /// What C tests of a scalar, as a branch tests an integer against zero.
    fn nonzero(&mut self, value: i64) -> i64 {
        match self.shape(value) {
            Shape::Float(_) => {
                let zero = self.constant(self.type_of(value), Number::Float(0.0));
                let truth = self.types.of(Shape::Bool, None);
                self.op(Op::Ne, truth, vec![value_ref(value), value_ref(zero)])
            }
            Shape::Pointer(4) => {
                let dword = self.types.int(4, false);
                self.op(Op::Convert, dword, vec![value_ref(value)])
            }
            Shape::Pointer(_) => {
                let null = self.constant(self.type_of(value), Number::Int(0));
                let truth = self.types.of(Shape::Bool, None);
                self.op(Op::Ne, truth, vec![value_ref(value), value_ref(null)])
            }
            _ => value,
        }
    }

    fn compare(&mut self, args: &[String]) -> R<i64> {
        let [cg_op, left, right, type_] = args else { return self.refuse("a compare of no three operands") };
        let a = self.value_as(left, type_)?;
        let b = self.value_as(right, type_)?;
        let truth = self.types.of(Shape::Bool, None);
        if is_float(&self.unit.canonical_type(type_)) {
            let op = match cg_op.as_str() {
                "O_EQ" => Op::Eq,
                "O_NE" => Op::Ne,
                "O_LT" => Op::Lt,
                "O_LE" => Op::Le,
                "O_GT" => Op::Gt,
                "O_GE" => Op::Ge,
                other => return self.refuse(format!("float compare {other}")),
            };
            return Ok(self.op(op, truth, vec![value_ref(a), value_ref(b)]));
        }
        // A far pointer compares as its dword, as the old raise does.
        let (a, b) = match self.space(a) {
            Some(FAR) => {
                let dword = self.types.int(4, false);
                (self.op(Op::Convert, dword, vec![value_ref(a)]), self.op(Op::Convert, dword, vec![value_ref(b)]))
            }
            _ => (a, b),
        };
        let is_signed = signed(&self.unit.canonical_type(type_));
        let op = match (cg_op.as_str(), is_signed) {
            ("O_EQ", _) => Op::Eq,
            ("O_NE", _) => Op::Ne,
            ("O_LT", true) => Op::Lt,
            ("O_LE", true) => Op::Le,
            ("O_GT", true) => Op::Gt,
            ("O_GE", true) => Op::Ge,
            ("O_LT", false) => Op::Below,
            ("O_LE", false) => Op::BelowEq,
            ("O_GT", false) => Op::Above,
            ("O_GE", false) => Op::AboveEq,
            (other, _) => return self.refuse(format!("compare {other}")),
        };
        Ok(self.op(op, truth, vec![value_ref(a), value_ref(b)]))
    }

    // ---- expressions ----

    fn eval(&mut self, node: &str) -> R<Got> {
        let key = hir::handle(node);
        if let Some(&done) = self.done.get(&key) {
            return Ok(done);
        }
        let got = self.expression(node)?;
        self.done.insert(key, got);
        Ok(got)
    }

    fn type_of_node(&self, node: &str) -> String {
        let tree = &self.unit.nodes[&hir::handle(node)];
        match tree.call.as_str() {
            "CGCall" => self.unit.calls[&hir::handle(&tree.args[0])].type_.clone(),
            "CGEval" | "CGVolatile" | "CGAttr" => self.type_of_node(&tree.args[0]),
            "CGFlow" | "CGCompare" => "TY_BOOLEAN".to_owned(),
            _ => tree.args.last().cloned().unwrap_or_default(),
        }
    }

    /// `node`'s scalar value as `type_`.
    fn value_as(&mut self, node: &str, type_: &str) -> R<i64> {
        let got = self.eval(node)?;
        let value = self.scalar(got)?;
        let from = self.type_of_node(node);
        self.converted(value, &from, type_)
    }

    /// A node's result used as a value: an address is the pointer, a
    /// function decays to its address, a call's result is itself.
    fn scalar(&mut self, got: Got) -> R<i64> {
        match got {
            Got::Value(value) | Got::Volatile(value) | Got::Aggregate(value, _) | Got::Returned(Some(value)) => Ok(value),
            Got::Function(symbol) => {
                let symbol = &self.unit.symbols[&symbol];
                if inline(symbol) {
                    return self.refuse(format!("an address of inline code {}", symbol.name));
                }
                let ty = self.types.pointer(space(self.unit, Key::Symbol(symbol.id)));
                let value = self.value(ty);
                self.instruction(Op::Address, vec![value], Vec::new()).callee = Some(symbol.object_name());
                Ok(value)
            }
            Got::Returned(None) => self.refuse("a void call's value"),
        }
    }

    /// An lvalue's address, and whether it is accessed volatile.
    fn address(&mut self, got: Got) -> R<(i64, bool)> {
        let (value, volatile) = match got {
            Got::Volatile(value) => (value, true),
            got => (self.scalar(got)?, false),
        };
        match (self.space(value), self.bits(value)) {
            (Some(_), _) => Ok((value, volatile)),
            (None, Some(32)) => {
                let far = self.types.pointer(FAR);
                Ok((self.op(Op::Convert, far, vec![value_ref(value)]), volatile))
            }
            (None, Some(_)) => {
                let word = self.types.int(2, false);
                let value = self.resized(value, false, word);
                let near = self.types.pointer(0);
                Ok((self.op(Op::Convert, near, vec![value_ref(value)]), volatile))
            }
            _ => self.refuse("a float used as an address"),
        }
    }

    fn expression(&mut self, node: &str) -> R<Got> {
        let tree = &self.unit.nodes[&hir::handle(node)];
        let args: Vec<&str> = tree.args.iter().map(String::as_str).collect();
        let call = tree.call.as_str();
        Ok(match (call, &args[..]) {
            ("CGInteger" | "CGInt64", [value, type_]) => {
                let value: BigInt = value.trim().parse().map_err(|_| Unsupported(format!("not an integer: {value}")))?;
                let ty = self.ty(type_)?;
                let wrapped = |width: i64| (value & ((BigInt::from(1) << (width * 8)) - BigInt::from(1))).to_u64().expect("a wrapped integer fits") as i64;
                match self.types.shape(ty) {
                    Shape::Int(width, _) => Got::Value(self.constant(ty, Number::Int(wrapped(width)))),
                    // A pointer constant is an integer as wide: a far one segment:offset.
                    Shape::Pointer(width) => {
                        let int = self.types.int(width, false);
                        let value = self.constant(int, Number::Int(wrapped(width)));
                        Got::Value(self.converted(value, &format!("TY_UINT_{width}"), type_)?)
                    }
                    other => return self.refuse(format!("an integer constant as {other:?}")),
                }
            }
            ("CGFloat", [text, type_]) if is_float(type_) => {
                let value: f64 = text.trim().trim_matches('"').parse().map_err(|_| Unsupported(format!("not a float: {text}")))?;
                let ty = self.ty(type_)?;
                Got::Value(self.constant(ty, Number::Float(value)))
            }
            ("CGFEName", [symbol, _]) => self.named(symbol)?,
            ("CGTempName", [temp, _]) => match self.slots.get(*temp) {
                Some(&slot) => Got::Value(self.address_of(slot)),
                None => return self.refuse(format!("no temporary {temp}")),
            },
            ("CGBackName", [back, _]) => match back_key(self.unit, hir::handle(back)) {
                Key::Symbol(symbol) => self.named(&format!("y{symbol}"))?,
                key => match self.shared.keys.get(&key) {
                    Some(&object) => {
                        let place = self.global(object);
                        Got::Value(self.address_of(place))
                    }
                    None => return self.refuse(format!("literal {back} is in no data segment")),
                },
            },
            ("CGUnary", ["O_POINTS", inner, type_]) => {
                let got = self.eval(inner)?;
                self.points(got, type_)?
            }
            ("CGUnary", ["O_CONVERT", inner, type_]) => Got::Value(self.value_as(inner, type_)?),
            ("CGUnary", [cg_op, inner, type_]) if library_routine(cg_op).is_some() => {
                let argument = self.value_as(inner, "TY_DOUBLE")?;
                let result = self.routine(cg_op, &[argument])?;
                Got::Value(self.converted(result, "TY_DOUBLE", type_)?)
            }
            ("CGUnary", [cg_op, inner, type_]) => {
                let value = self.value_as(inner, type_)?;
                Got::Value(self.unary(cg_op, value, type_)?)
            }
            ("CGBinary", ["O_COMMA", left, right, _]) => {
                self.eval(left)?;
                self.eval(right)?
            }
            ("CGBinary", [cg_op, left, right, type_]) if library_routine(cg_op).is_some() => {
                // The runtime takes them last first, as every call's parms are listed.
                let second = self.value_as(right, "TY_DOUBLE")?;
                let first = self.value_as(left, "TY_DOUBLE")?;
                let result = self.routine(cg_op, &[first, second])?;
                Got::Value(self.converted(result, "TY_DOUBLE", type_)?)
            }
            ("CGBinary", [cg_op, left, right, type_]) => Got::Value(self.binary(cg_op, left, right, type_)?),
            // Watcom emits an aggregate assignment as either.
            ("CGAssign", [target, source, type_]) if self.types.aggregate(type_).is_none() => {
                let value = self.value_as(source, type_)?;
                let got = self.eval(target)?;
                let (pointer, volatile) = self.address(got)?;
                self.store(value, pointer, volatile, type_)?;
                Got::Value(value)
            }
            ("CGAssign" | "CGLVAssign", [target, source, _]) => {
                let Got::Aggregate(from, size) = self.eval(source)? else { return self.refuse("an aggregate assignment from a scalar") };
                let got = self.eval(target)?;
                let (into, _) = self.address(got)?;
                self.copy(into, from, size);
                Got::Aggregate(into, size)
            }
            ("CGPostGets" | "CGPreGets", [cg_op, target, source, type_]) => {
                let got = self.eval(target)?;
                let (pointer, volatile) = self.address(got)?;
                let old = self.load(pointer, volatile, type_)?;
                let new = if self.space(old).is_some() {
                    let from = self.type_of_node(source);
                    let got = self.eval(source)?;
                    let by = self.scalar(got)?;
                    self.moved(old, by, &from, *cg_op == "O_MINUS")?
                } else {
                    let by = self.value_as(source, type_)?;
                    self.arithmetic(cg_op, old, by, type_)?
                };
                self.store(new, pointer, volatile, type_)?;
                Got::Value(if call == "CGPostGets" { old } else { new })
            }
            ("CGCall", [call]) => {
                let call = &self.unit.calls[&hir::handle(call)];
                self.call(call)?
            }
            ("CGChoose", [test, yes, no, type_]) => {
                // A local both arms store, which promotion makes a phi.
                let ty = self.ty(type_)?;
                let width = self.types.get(ty).width;
                let chosen = self.local(&format!("n{}", hir::handle(node)), ty, width);
                let otherwise = self.block();
                let join = self.block();
                self.branch(test, otherwise, false)?;
                let a = self.value_as(yes, type_)?;
                self.instruction(Op::Store, Vec::new(), vec![Operand::place_ref(chosen), value_ref(a)]);
                self.jump(join);
                self.position(otherwise);
                let b = self.value_as(no, type_)?;
                self.instruction(Op::Store, Vec::new(), vec![Operand::place_ref(chosen), value_ref(b)]);
                self.jump(join);
                self.position(join);
                Got::Value(self.op(Op::Load, ty, vec![Operand::place_ref(chosen)]))
            }
            ("CGCompare", _) => Got::Value(self.compare(&tree.args)?),
            ("CGFlow", _) => {
                let truth = self.types.of(Shape::Bool, None);
                let flowed = self.local(&format!("n{}", hir::handle(node)), truth, 2);
                let no = self.block();
                let join = self.block();
                self.branch(node, no, false)?;
                let one = self.constant(truth, Number::Int(1));
                self.instruction(Op::Store, Vec::new(), vec![Operand::place_ref(flowed), value_ref(one)]);
                self.jump(join);
                self.position(no);
                let zero = self.constant(truth, Number::Int(0));
                self.instruction(Op::Store, Vec::new(), vec![Operand::place_ref(flowed), value_ref(zero)]);
                self.jump(join);
                self.position(join);
                Got::Value(self.op(Op::Load, truth, vec![Operand::place_ref(flowed)]))
            }
            ("CGEval", [inner]) | ("CGAttr", [inner, _]) => self.eval(inner)?,
            ("CGVolatile", [inner]) => {
                let got = self.eval(inner)?;
                Got::Volatile(self.address(got)?.0)
            }
            _ => return self.refuse(format!("{} {}", tree.call, tree.args.join(" "))),
        })
    }

    /// A symbol's lvalue: its slot's address, its global's, or the function.
    fn named(&mut self, token: &str) -> R<Got> {
        if let Some(&slot) = self.slots.get(token) {
            return Ok(Got::Value(self.address_of(slot)));
        }
        let symbol = &self.unit.symbols[&hir::handle(token)];
        if symbol.proc() {
            return Ok(Got::Function(symbol.id));
        }
        match self.shared.keys.get(&Key::Symbol(symbol.id)) {
            Some(&object) => {
                let place = self.global(object);
                Ok(Got::Value(self.address_of(place)))
            }
            None => self.refuse(format!("{} is neither defined nor imported", symbol.name)),
        }
    }

    /// O_POINTS: what `got` addresses, read as `type_`.
    fn points(&mut self, got: Got, type_: &str) -> R<Got> {
        if let Got::Returned(value) = got {
            let Some(value) = value else { return self.refuse("a void call's value") };
            return Ok(Got::Value(value));
        }
        if let Some(size) = self.types.aggregate(type_) {
            if let Got::Aggregate(_, had) = got
                && had != size
            {
                return self.refuse(format!("{had}-byte aggregate used as {size} bytes"));
            }
            let (pointer, _) = self.address(got)?;
            return Ok(Got::Aggregate(pointer, size));
        }
        let (pointer, volatile) = self.address(got)?;
        Ok(Got::Value(self.load(pointer, volatile, type_)?))
    }

    fn load(&mut self, pointer: i64, volatile: bool, type_: &str) -> R<i64> {
        let ty = self.ty(type_)?;
        Ok(self.op(Op::Load, ty, vec![Operand::IndirectPlace(indirect(pointer, 0, ty, volatile))]))
    }

    fn store(&mut self, value: i64, pointer: i64, volatile: bool, type_: &str) -> R<()> {
        let ty = self.ty(type_)?;
        self.instruction(Op::Store, Vec::new(), vec![Operand::IndirectPlace(indirect(pointer, 0, ty, volatile)), value_ref(value)]);
        Ok(())
    }

    /// `size` bytes from `from` to `into`, widest first, as the old raise copies.
    fn copy(&mut self, into: i64, from: i64, size: i64) {
        let mut done = 0;
        while done < size {
            let width = [4, 2, 1].into_iter().find(|&one| size - done >= one).expect("a byte left");
            let ty = self.types.raw(width);
            let source = self.displaced(from, done);
            let target = self.displaced(into, done);
            let moved = self.op(Op::Load, ty, vec![Operand::IndirectPlace(indirect(source, 0, ty, false))]);
            self.instruction(Op::Store, Vec::new(), vec![Operand::IndirectPlace(indirect(target, 0, ty, false)), value_ref(moved)]);
            done += width;
        }
    }

    /// `pointer` moved `by` constant bytes, inside its object.
    fn displaced(&mut self, pointer: i64, by: i64) -> i64 {
        if by == 0 {
            return pointer;
        }
        let at = self.int(2, by);
        let result = self.value(self.type_of(pointer));
        self.instruction(Op::PtrOffset, vec![result], vec![value_ref(pointer), value_ref(at)]).inbounds = true;
        result
    }

    /// `value`, of C type `from`, as `to`.
    fn converted(&mut self, value: i64, from: &str, to: &str) -> R<i64> {
        let ty = self.ty(to)?;
        if self.same(self.type_of(value), ty) {
            return Ok(value);
        }
        let source_signed = signed(&self.unit.canonical_type(from));
        Ok(match (self.shape(value), self.types.shape(ty)) {
            (Shape::Int(..) | Shape::Bool, Shape::Int(..)) => self.resized(value, source_signed, ty),
            (Shape::Int(..) | Shape::Bool, Shape::Float(_)) => {
                let bits = self.bits(value).expect("an integer");
                if source_signed {
                    let value = self.as_signed(value, true);
                    self.op(Op::Convert, ty, vec![value_ref(value)])
                } else if bits < 64 {
                    // No unsigned load: its zero extension, one width up, is signed.
                    let wide = self.types.int(if bits < 32 { 4 } else { 8 }, true);
                    let wide = self.resized(value, false, wide);
                    let wide = self.as_signed(wide, true);
                    self.op(Op::Convert, ty, vec![value_ref(wide)])
                } else {
                    let value = self.as_signed(value, false);
                    self.op(Op::Convert, ty, vec![value_ref(value)])
                }
            }
            // Toward zero; the result's signedness picks the cast.
            (Shape::Float(_), Shape::Int(..)) => self.op(Op::Truncate, ty, vec![value_ref(value)]),
            (Shape::Float(_), Shape::Float(_)) => self.op(Op::Convert, ty, vec![value_ref(value)]),
            (Shape::Pointer(a), Shape::Pointer(b)) if a == b => value,
            (Shape::Pointer(_), Shape::Pointer(_)) => self.op(Op::Convert, ty, vec![value_ref(value)]),
            // An integer is the pointer's own bits, extended to its width as
            // Borland does: a far one is segment:offset, 0 the null pointer.
            (Shape::Int(..) | Shape::Bool, Shape::Pointer(width)) => {
                let int = self.types.int(width, false);
                let value = self.resized(value, source_signed, int);
                self.op(Op::Convert, ty, vec![value_ref(value)])
            }
            (Shape::Pointer(width), Shape::Int(..)) => {
                let int = self.types.int(width, false);
                let whole = self.op(Op::Convert, int, vec![value_ref(value)]);
                self.resized(whole, false, ty)
            }
            (source, target) => return self.refuse(format!("a conversion from {source:?} to {target:?}")),
        })
    }

    /// An integer at another width: extended as `is_signed` says, or truncated.
    fn resized(&mut self, value: i64, is_signed: bool, ty: i64) -> i64 {
        let (from, to) = (self.bits(value).expect("an integer"), self.types.get(ty).width * 8);
        match from.cmp(&to) {
            std::cmp::Ordering::Equal => value,
            std::cmp::Ordering::Less => {
                let value = self.as_signed(value, is_signed);
                self.op(if is_signed { Op::SignExtend } else { Op::ZeroExtend }, ty, vec![value_ref(value)])
            }
            std::cmp::Ordering::Greater => self.op(Op::Convert, ty, vec![value_ref(value)]),
        }
    }

    fn unary(&mut self, cg_op: &str, value: i64, type_: &str) -> R<i64> {
        let floats = is_float(&self.unit.canonical_type(type_));
        let ty = self.type_of(value);
        Ok(match (cg_op, floats) {
            ("O_UMINUS", true) => self.op(Op::Fneg, ty, vec![value_ref(value)]),
            ("O_FABS", true) => self.op(Op::Fabs, ty, vec![value_ref(value)]),
            ("O_UMINUS", false) => {
                let bits = self.bits(value).ok_or_else(|| Unsupported(format!("{}: negation of a non-integer", self.name())))?;
                let nowrap = self.wraps(type_, bits);
                let result = self.value(ty);
                self.instruction(Op::Neg, vec![result], vec![value_ref(value)]).nowrap = nowrap;
                result
            }
            ("O_COMPLEMENT", false) => {
                if self.bits(value).is_none() {
                    return self.refuse("complement of a non-integer");
                }
                self.op(Op::Not, ty, vec![value_ref(value)])
            }
            (other, _) => return self.refuse(format!("{other} of {type_}")),
        })
    }

    /// Whether C promises a signed result fits: an int or wider.
    fn wraps(&self, type_: &str, bits: i64) -> bool {
        signed(&self.unit.canonical_type(type_)) && bits >= 16
    }

    fn binary(&mut self, cg_op: &str, left: &str, right: &str, type_: &str) -> R<i64> {
        let canonical = self.unit.canonical_type(type_);
        if matches!(cg_op, "O_PLUS" | "O_MINUS") && !is_float(&canonical) {
            let a_got = self.eval(left)?;
            let b_got = self.eval(right)?;
            let (a, b) = (self.scalar(a_got)?, self.scalar(b_got)?);
            // Pointer arithmetic: the pointer moved, or two pointers' distance.
            match (self.space(a).is_some(), self.space(b).is_some()) {
                (true, false) => {
                    let from = self.type_of_node(right);
                    let moved = self.moved(a, b, &from, cg_op == "O_MINUS")?;
                    return self.converted(moved, &self.type_of_node(left), type_);
                }
                (false, true) if cg_op == "O_PLUS" => {
                    let from = self.type_of_node(left);
                    let moved = self.moved(b, a, &from, false)?;
                    return self.converted(moved, &self.type_of_node(right), type_);
                }
                (true, true) if cg_op == "O_MINUS" && !pointers(&canonical) => {
                    let a = self.converted(a, &self.type_of_node(left), type_)?;
                    let b = self.converted(b, &self.type_of_node(right), type_)?;
                    let ty = self.ty(type_)?;
                    return Ok(self.op(Op::Sub, ty, vec![value_ref(a), value_ref(b)]));
                }
                _ if pointers(&canonical) => {
                    // Integer arithmetic typed as a pointer: a displacement from its first operand.
                    let a = self.converted(a, &self.type_of_node(left), type_)?;
                    let from = self.type_of_node(right);
                    return self.moved(a, b, &from, cg_op == "O_MINUS");
                }
                _ => {}
            }
        }
        let a = self.value_as(left, type_)?;
        let b = if matches!(cg_op, "O_LSHIFT" | "O_RSHIFT") {
            // HIR's MIR takes a shift count to the shifted value's width.
            let got = self.eval(right)?;
            let count = self.scalar(got)?;
            if self.bits(a).is_none() || self.bits(count).is_none() {
                return self.refuse(format!("a shift of {type_}"));
            }
            count
        } else {
            self.value_as(right, type_)?
        };
        self.arithmetic(cg_op, a, b, type_)
    }

    /// `pointer` moved by `by` bytes, of C type `from`: inbounds, as C
    /// keeps pointer arithmetic inside its object.
    fn moved(&mut self, pointer: i64, by: i64, from: &str, subtract: bool) -> R<i64> {
        let word = self.types.int(2, true);
        let by = match self.bits(by) {
            Some(_) => self.resized(by, signed(&self.unit.canonical_type(from)), word),
            None => return self.refuse(format!("a pointer moved by a {from}")),
        };
        let by = if subtract { self.op(Op::Neg, word, vec![value_ref(by)]) } else { by };
        let ty = self.type_of(pointer);
        let result = self.value(ty);
        self.instruction(Op::PtrOffset, vec![result], vec![value_ref(pointer), value_ref(by)]).inbounds = true;
        Ok(result)
    }

    fn arithmetic(&mut self, cg_op: &str, a: i64, b: i64, type_: &str) -> R<i64> {
        let ty = self.ty(type_)?;
        if is_float(&self.unit.canonical_type(type_)) {
            let op = match cg_op {
                "O_PLUS" => Op::Fadd,
                "O_MINUS" => Op::Fsub,
                "O_TIMES" => Op::Fmul,
                "O_DIV" => Op::Fdiv,
                other => return self.refuse(format!("float {other}")),
            };
            return Ok(self.op(op, ty, vec![value_ref(a), value_ref(b)]));
        }
        let Some(bits) = self.bits(a) else { return self.refuse(format!("{cg_op} of {type_}")) };
        let is_signed = signed(&self.unit.canonical_type(type_));
        let (op, nowrap) = match cg_op {
            "O_PLUS" => (Op::Add, self.wraps(type_, bits)),
            "O_MINUS" => (Op::Sub, self.wraps(type_, bits)),
            "O_TIMES" => (Op::Mul, self.wraps(type_, bits)),
            "O_AND" => (Op::And, false),
            "O_OR" => (Op::Or, false),
            "O_XOR" => (Op::Xor, false),
            "O_LSHIFT" => (Op::Shl, false),
            "O_RSHIFT" => (if is_signed { Op::Sar } else { Op::Shr }, false),
            "O_DIV" | "O_MOD" => {
                let op = match (cg_op, is_signed) {
                    ("O_DIV", true) => Op::Div,
                    ("O_DIV", false) => Op::Udiv,
                    (_, true) => Op::Rem,
                    (_, false) => Op::Urem,
                };
                if bits < 16 {
                    // A byte's quotient at a word, as C promotes one.
                    let word = self.types.int(2, is_signed);
                    let (x, y) = (self.resized(a, is_signed, word), self.resized(b, is_signed, word));
                    let wide = self.op(op, word, vec![value_ref(x), value_ref(y)]);
                    return Ok(self.op(Op::Convert, ty, vec![value_ref(wide)]));
                }
                (op, false)
            }
            other => return self.refuse(other.to_owned()),
        };
        let result = self.value(ty);
        self.instruction(op, vec![result], vec![value_ref(a), value_ref(b)]).nowrap = nowrap;
        Ok(result)
    }

    /// A library routine's double result for an operator: Borland's
    /// library, far and cdecl, doubles in and out.
    fn routine(&mut self, cg_op: &str, arguments: &[i64]) -> R<i64> {
        let name = format!("_{}", library_routine(cg_op).expect("a routine"));
        self.callable(&name);
        let double = self.ty("TY_DOUBLE")?;
        let result = self.value(double);
        let order = (0..arguments.len() as i64).rev().collect();
        self.call_site(Some(&name), Some(result), arguments.iter().map(|&one| value_ref(one)).collect(), order, StackCleanup::Caller, CallDistance::Far);
        Ok(result)
    }

    /// Declares a procedure the unit calls by `name`.
    fn callable(&mut self, name: &str) {
        callable(self.callables, name, false);
    }

    /// A call of `callee`, or of the function `operands[0]` points to.
    fn call_site(&mut self, callee: Option<&str>, result: Option<i64>, operands: Vec<Operand>, order: Vec<i64>, cleanup: StackCleanup, distance: CallDistance) {
        let results = result.into_iter().collect();
        let instruction = self.instruction(Op::Call, results, operands);
        instruction.callee = callee.map(str::to_owned);
        let id = instruction.id;
        self.calls.push(h::CallAbi { instruction: id, order, cleanup, distance, callee: None, float_return: FloatReturn::Register, promises: Vec::new() });
    }

    /// Inline code as a call of `llrm.ia16.code`, each frame place it names
    /// an argument; `__emit__`'s are its constant arguments' bytes.
    fn inline_code(&mut self, call: &hir::Call, symbol: &hir::Symbol) -> R<Got> {
        let code = match &symbol.code {
            Some(code) => code.clone(),
            None => {
                let byte = |node: &String| match self.unit.nodes.get(&hir::handle(node)).map(|tree| (tree.call.as_str(), tree.args.first())) {
                    Some(("CGInteger", Some(value))) => u8::try_from(hir::int(value)).ok(),
                    _ => None,
                };
                let Some(data) = call.parms.iter().rev().map(|(node, _)| byte(node)).collect::<Option<Vec<u8>>>() else {
                    return self.refuse(format!("{} of anything but constant bytes", symbol.name));
                };
                hir::Code { data, fixups: Vec::new() }
            }
        };
        let mut places = Vec::new();
        let mut operands = Vec::new();
        for fixup in &code.fixups {
            let key = format!("y{}", fixup.symbol);
            let (Some(&slot), "offset") = (self.slots.get(&key), fixup.kind.as_str()) else {
                return self.refuse(format!("inline code's {} of {}", fixup.kind, self.unit.symbols[&fixup.symbol].name));
            };
            places.push((fixup.at as usize, fixup.offset));
            operands.push(value_ref(self.address_of(slot)));
        }
        let name = llrm_mir::intrinsics::code_name(&code.data, &places);
        self.callable(&name);
        let result = self.types.int(4, false);
        let result = self.value(result);
        let order = (0..operands.len() as i64).rev().collect();
        self.call_site(Some(&name), Some(result), operands, order, StackCleanup::Caller, CallDistance::Near);
        self.inlined = Some(result);
        Ok(Got::Returned(Some(result)))
    }

    fn call(&mut self, call: &hir::Call) -> R<Got> {
        let target = self.eval(&call.target)?;
        let unit = self.unit;
        if let Got::Function(symbol) = target
            && inline(&unit.symbols[&symbol])
        {
            return self.inline_code(call, &unit.symbols[&symbol]);
        }
        let symbol = match target {
            Got::Function(symbol) => &unit.symbols[&symbol],
            _ => &unit.symbols[&call.symbol],
        };
        // bcc -O's builtin: the library's fabs is the x87 instruction.
        if let (Got::Function(_), [(node, type_)]) = (target, call.parms.as_slice())
            && symbol.name == "fabs"
            && symbol.imported()
            && is_float(&unit.canonical_type(type_))
            && is_float(&unit.canonical_type(&call.type_))
        {
            let value = self.value_as(node, type_)?;
            return Ok(Got::Returned(Some(self.unary("O_FABS", value, type_)?)));
        }
        let cleanup = cleanup(symbol).map_err(|error| Unsupported(format!("{}: {}", self.name(), error.0)))?;
        let distance = distance(symbol);
        let callee = match target {
            Got::Function(_) => None,
            other => Some(self.scalar(other)?),
        };
        // Evaluated as listed, last first; passed first first.
        let mut arguments = Vec::new();
        for (node, type_) in &call.parms {
            let Some(size) = self.types.aggregate(type_) else {
                arguments.push(self.value_as(node, type_)?);
                continue;
            };
            if cleanup != StackCleanup::Caller {
                return self.refuse("an aggregate argument by a callee-pops convention");
            }
            let Got::Aggregate(from, _) = self.eval(node)? else { return self.refuse("a scalar passed as an aggregate") };
            // Its words, last first as the arguments are listed; a last odd byte widened.
            let word = self.types.raw(2);
            for at in (0..(size + 1) / 2).rev().map(|one| one * 2) {
                let value = if at + 1 < size {
                    self.op(Op::Load, word, vec![Operand::IndirectPlace(indirect(from, at, word, false))])
                } else {
                    let byte = self.types.raw(1);
                    let value = self.op(Op::Load, byte, vec![Operand::IndirectPlace(indirect(from, at, byte, false))]);
                    self.op(Op::ZeroExtend, word, vec![value_ref(value)])
                };
                arguments.push(value);
            }
        }
        arguments.reverse();
        let result = match target {
            Got::Function(symbol) if self.shared.valueless.contains(&symbol) => None,
            _ => {
                let ty = self.ty(&call.type_)?;
                Some(self.value(ty))
            }
        };
        let count = arguments.len() as i64;
        let order = if cleanup == StackCleanup::Caller { (0..count).rev().collect() } else { (0..count).collect() };
        let operands: Vec<Operand> = callee.into_iter().chain(arguments).map(value_ref).collect();
        let name = match target {
            Got::Function(_) => {
                let name = symbol.object_name();
                if !self.unit.procs.iter().any(|one| one.symbol == symbol.id) {
                    self.callable(&name);
                }
                Some(name)
            }
            _ => None,
        };
        self.call_site(name.as_deref(), result, operands, order, cleanup, distance);
        Ok(Got::Returned(result))
    }
}

/// `name`'s callable, registered on first sight, and its id.
fn callable(callables: &mut IndexMap<String, h::Callable>, name: &str, defined: bool) -> i64 {
    let id = callables.len() as i64 + 1;
    callables
        .entry(name.to_owned())
        .or_insert_with(|| h::Callable {
            id,
            name: name.to_owned(),
            result_type: None,
            parameter_types: Vec::new(),
            by_value: Vec::new(),
            segmented: Vec::new(),
            arrays: Vec::new(),
            defined,
        })
        .id
}

/// An access through `base`, `offset` bytes in, as `ty`.
fn indirect(base: i64, offset: i64, ty: i64, volatile: bool) -> h::IndirectPlace {
    h::IndirectPlace { base, offset, r#type: ty, volatile, published: false, inbounds: true, origin: None, allocation: None }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use llrm_mir::{Attribute, Linkage, Module};

    use crate::{hir, stream};

    fn raised(fixture: &str) -> Module {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c").join(fixture)).unwrap();
        let program = super::program(&hir::unit(&stream::parse(&text)).unwrap(), "test").unwrap();
        let emitted = llrm_core::hir::mir::emit(&program).swap_remove(0);
        assert_eq!(emitted.refused, Vec::<(String, String)>::new());
        emitted.module
    }

    /// `@name`'s definition as printed.
    fn defined(module: &Module, name: &str) -> String {
        let text = llrm_mir::print::module(module);
        let start = text[..text.find(&format!("@{name}(")).unwrap()].rfind("define").unwrap();
        let length = text[start..].find("\n}").unwrap();
        text[start..start + length].to_owned()
    }

    /// The `!tbaa` tag node of aliasing class `class`, as printed.
    fn tag(printed: &str, class: &str) -> String {
        let node = printed.lines().find(|one| one.contains(&format!("!\"{class}\""))).unwrap();
        let node = &node[..node.find(' ').unwrap()];
        let tag = printed.lines().find(|one| one.contains(&format!("= !{{{node}, {node}, i64 0}}"))).unwrap();
        tag[..tag.find(' ').unwrap()].to_owned()
    }

    /// C99 6.7.3.1: the three restrict parameters of `add` reach distinct objects.
    #[test]
    fn test_restrict_parameters_are_noalias() {
        let module = raised("tests/test_restrict_reaches_mir_as_distinct_noalias_roots.cgs");
        let function = module.global(module.named("_add").unwrap()).function().unwrap();
        let noalias = Attribute::Flag("noalias".to_owned());
        assert!(function.parameter_attrs.iter().all(|one| one.contains(&noalias)), "{:?}", function.parameter_attrs);
    }

    #[test]
    fn test_static_is_internal_linkage() {
        let module = raised("iparg.cgs");
        let linkage = |module: &Module, name| module.global(module.named(name).unwrap()).linkage;
        assert_eq!((linkage(&module, "_twice"), linkage(&module, "_answer_from_argument")), (Linkage::Internal, Linkage::External));
        assert_eq!(linkage(&raised("parity/loop.cgs"), "_demo_seed"), Linkage::Internal);
    }

    /// In `bytes`, `i++` is a signed int's and `total +=` an unsigned's;
    /// `a[i]` stays inside `a` and reads a character, which aliases anything.
    #[test]
    fn test_signed_arithmetic_is_nsw_and_pointer_arithmetic_inbounds() {
        let module = raised("bytes.cgs");
        let text = defined(&module, "_bytes");
        let adds: Vec<&str> = text.lines().filter(|one| one.contains(" = add ")).collect();
        assert!(adds.iter().any(|one| one.contains("add nsw i16")) && adds.iter().any(|one| one.contains("add i16")), "{adds:#?}");
        assert!(text.lines().filter(|one| one.contains("getelementptr")).all(|one| one.contains("getelementptr inbounds")), "{text}");
        let char = tag(&llrm_mir::print::module(&module), "omnipotent char");
        assert!(text.lines().filter(|one| one.contains("load i8")).all(|one| one.ends_with(&format!("!tbaa {char}"))), "{text}");
    }

    /// `*seed` is a short's access: C's int2 class, not the character type.
    #[test]
    fn test_accesses_carry_their_aliasing_class() {
        let module = raised("parity/loop.cgs");
        let text = defined(&module, "_parity_loop");
        let int2 = tag(&llrm_mir::print::module(&module), "int2");
        assert!(text.lines().any(|one| one.contains("load i16") && one.ends_with(&format!("!tbaa {int2}"))), "{text}");
    }
}
