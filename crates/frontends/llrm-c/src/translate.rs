//! One unit's trees as llrm's HIR: each C object a place or a data object,
//! each tree node its instructions. `hir::mir` makes the MIR.
//!
//! What C promises is marked in the HIR: its aliasing classes, `restrict`,
//! internal linkage for what the unit does not export, `nowrap` on signed
//! arithmetic, `inbounds` on pointer arithmetic. The last two hold for
//! objects of at most PTRDIFF_MAX bytes, as clang assumes of every object.

use std::collections::{HashMap, HashSet};

use llrm_core::hir::facts::{Builder as Facts, Subject};
use llrm_mir::facts::Fact;
use llrm_core::hir::model::{
    self as h, AddressKind, CallDistance, DataLinkage, FloatEvaluation, FloatReturn, Number, Op, Operand, StackCleanup, Storage, TerminatorKind, TypeKind,
};
use llrm_core::support::hash::IndexMap;
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::hir::{self, Unsupported};
use crate::raise_hir::{EMITTED, classes, far_pointers, is_float, library_routine, pointers, signed, widths, classes_for, widths_for};

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
    let mut facts = Facts::new("c");
    for object in &objects {
        data.push(data_object(unit, object, keys[&object.key], &keys, &mut callables)?);
        if let Some(align) = object.align {
            facts.state(Subject::Object(keys[&object.key]), Fact::Align(align));
        }
    }
    for symbol in imports {
        let id = keys[&Key::Symbol(symbol.id)];
        data.push(h::DataObject {
            linkage: DataLinkage::External,
            address: address(space(unit, Key::Symbol(symbol.id))),
            // An extern const object: writing it, anywhere, is undefined.
            readonly: symbol.constant(),
            addressed: !symbol.unaddressed(unit.switches),
            ..h::DataObject::new(id, &symbol.object_name(), Vec::new())
        });
    }
    let valueless: HashSet<i64> = unit
        .procs
        .iter()
        .filter(|proc| proc.body.iter().filter(|one| one.call == "CGReturn").all(|one| one.args[0] == "n0") && !answered_by_inline_code(unit, proc))
        .map(|proc| proc.symbol)
        .collect();
    let module = Shared { unit, data: &data, keys: &keys, valueless: &valueless };
    let mut described = crate::debug::Described::of(unit);
    let mut functions = Vec::new();
    for (at, proc) in unit.procs.iter().enumerate() {
        functions.push(Body::function(&module, &mut types, &mut callables, &mut described, &mut facts, proc, at as i64 + 1)?);
    }
    let debug = described.map(|one| one.finish(|symbol| keys.get(&Key::Symbol(symbol)).copied()));
    let defined: HashSet<&str> = functions.iter().map(|one: &h::Function| one.name.as_str()).collect();
    let callables: Vec<h::Callable> = callables.into_values().filter(|one| one.defined || !defined.contains(one.name.as_str())).collect();
    let promises = h::RuntimePromises { reads_arguments: crate::libfunc::reads_arguments(callables.iter().map(|one| one.name.as_str())), ..Default::default() };
    let mut callables = callables;
    for symbol in unit.symbols.values().filter(|one| one.proc()) {
        if let Some(callable) = callables.iter_mut().find(|one| one.name == symbol.object_name()) {
            callable.returns_twice = crate::ow_facts::returns_twice(symbol.call_class) || crate::libfunc::returns_twice(&callable.name);
            for fact in crate::ow_facts::of_call_class(symbol.call_class)? {
                facts.state(Subject::Callable(callable.id), fact);
            }
            if !callable.defined && crate::libfunc::three_way_compare(&callable.name) {
                facts.state(Subject::Callable(callable.id), llrm_mir::facts::Fact::ThreeWayCompare);
            }
        }
    }
    let (types, alias_classes) = types.finished();
    let module = h::Module { data, callables, alias_classes, debug, facts: facts.finish(), ..h::Module::new(1, name, types, functions) };
    // Borland's medium model: a call keeps what its contract does not clobber;
    // the compiler's constants go in CONST.
    let contract = if unit.flat { crate::raise_hir::cdecl32(String::new(), true, 0) } else { crate::raise_hir::medium_model(String::new(), true, 0) };
    let preserved = llrm_core::abi::runtime::preserves(&contract);
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
/// A huge pointer's: a far one whose displacement carries into its selector.
const HUGE: u32 = 3;

/// Where a symbol or literal is: far unless in DGROUP.
fn space(unit: &hir::Unit, key: Key) -> u32 {
    match key {
        Key::Symbol(symbol) => {
            let symbol = &unit.symbols[&symbol];
            match (symbol.proc(), symbol.far(), unit.grouped(symbol)) {
                _ if unit.flat => 0,
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

/// What one segment holds of a huge object: the 8086's 64K.
const HUGE_SEGMENT: usize = 0x1_0000;

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
        let continues = out.last().is_some_and(|one| one.bytes.len() % HUGE_SEGMENT == 0 && !one.bytes.is_empty());
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
            // A huge object runs on into further segments, which hold no label: each full
            // 64K of it, and the rest in the last.
            let found = if continues && out.len() == first { out.last_mut() } else { out[first..].last_mut() };
            let Some(object) = found else { return refuse(format!("{} data before any label", segment.name)) };
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
                    let width = widths_for(unit.flat, type_).unwrap_or(2) as usize;
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
    let (linkage, readonly, addressed) = match object.key {
        Key::Symbol(symbol) => {
            let symbol = &unit.symbols[&symbol];
            (if symbol.exported() { DataLinkage::Exported } else { DataLinkage::Internal }, symbol.constant(), !symbol.unaddressed(unit.switches))
        }
        Key::Literal(_) => (DataLinkage::Private, true, true),
    };
    Ok(h::DataObject {
        readonly,
        addressed,
        relocations,
        linkage,
        address: address(space(unit, object.key)),
        segment: Some(object.segment.clone()),
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
    /// A far pointer whose displacement carries into its selector.
    Huge,
    Bytes(i64),
}

/// C's aliasing classes as clang's `!tbaa` tree: every class under the
/// character type, which may alias anything.
const ROOT: &str = "Simple C/C++ TBAA";
const CHAR: &str = "omnipotent char";
const CLASSES: [&str; 8] = ["int2", "int4", "int8", "float4", "float8", "float10", "pointer2", "pointer4"];

/// The module's types, each made once, by shape and aliasing class.
struct Types<'u> {
    unit: &'u hir::Unit,
    /// Flat 32-bit: `int` and every pointer 4 bytes, one address space.
    flat: bool,
    list: Vec<h::Type>,
    made: HashMap<(Shape, Option<&'static str>), i64>,
    members: HashMap<&'static str, Vec<i64>>,
}

impl<'u> Types<'u> {
    fn new(unit: &'u hir::Unit) -> Self {
        Types { unit, flat: unit.flat, list: Vec::new(), made: HashMap::new(), members: HashMap::new() }
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
            TypeKind::Pointer if one.address == AddressKind::Huge => Shape::Huge,
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
            Shape::Bool => (TypeKind::Boolean, self.word()),
            Shape::Float(width) => (TypeKind::Float, width),
            Shape::Pointer(width) => (TypeKind::Pointer, width),
            Shape::Huge => (TypeKind::Pointer, 4),
            Shape::Bytes(width) => (TypeKind::Opaque, width),
        };
        let mut made = h::Type::new(id, &format!("{shape:?}{}", class.map_or(String::new(), |one| format!(" {one}"))), kind, width);
        match shape {
            Shape::Int(_, is_signed) => made.signed = Some(is_signed),
            Shape::Bool => made.signed = Some(false),
            Shape::Float(4) => made.evaluation = FloatEvaluation::Binary32,
            Shape::Float(_) => made.evaluation = FloatEvaluation::Binary64,
            Shape::Pointer(width) => made.address = address(if width == 4 && !self.flat { FAR } else { 0 }),
            Shape::Huge => made.address = AddressKind::Huge,
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
        let near = self.word();
        self.of(if space == HUGE { Shape::Huge } else { Shape::Pointer(if space == FAR { 4 } else { near }) }, None)
    }

    /// An `int`'s bytes, which a near pointer and an index are too.
    fn word(&self) -> i64 {
        if self.flat { 4 } else { 2 }
    }

    /// An aggregate's size; none for a scalar.
    fn aggregate(&self, type_: &str) -> Option<i64> {
        self.unit.types.get(&self.unit.canonical_type(type_)).copied()
    }

    /// C type `type_` as a value, its accesses in its aliasing class.
    fn c(&mut self, type_: &str) -> R<i64> {
        let type_ = self.unit.canonical_type(type_);
        let big = |flag| if self.unit.target & flag != 0 { 4 } else { 2 };
        // A far code pointer is a far call; a far or huge data pointer is near where the target
        // has one address space, with a warning.
        if self.flat && type_ == "TY_LONG_CODE_PTR" {
            return refuse(format!("{type_} in flat code"));
        }
        if self.flat && matches!(type_.as_str(), "TY_LONG_POINTER" | "TY_HUGE_POINTER") {
            self.unit.warn_near();
        }
        let shape = match type_.as_str() {
            "TY_POINTER" | "TY_CODE_PTR" | "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" | "TY_LONG_POINTER" | "TY_HUGE_POINTER" if self.flat => Shape::Pointer(4),
            // An unmarked pointer is the target's near one, whatever model the front end was told.
            "TY_POINTER" => Shape::Pointer(2),
            "TY_CODE_PTR" => Shape::Pointer(big(hir::BIG_CODE)),
            "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => Shape::Pointer(2),
            "TY_HUGE_POINTER" => Shape::Huge,
            "TY_LONG_POINTER" | "TY_LONG_CODE_PTR" => Shape::Pointer(4),
            "TY_SINGLE" => Shape::Float(4),
            "TY_DOUBLE" => Shape::Float(8),
            "TY_LONG_DOUBLE" => Shape::Float(10),
            other => match (widths_for(self.flat, other), self.aggregate(other)) {
                (Some(width), _) => Shape::Int(i64::from(width), signed(other)),
                (None, Some(size)) => return refuse(format!("a {size}-byte aggregate as a value")),
                (None, None) => return refuse(format!("no MIR type for {other}")),
            },
        };
        let class = match type_.as_str() {
            "TY_POINTER" if !self.flat => Some("pointer2"),
            other => classes_for(self.flat, other),
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
    /// A bit field: `width` bits from `start` of the unit of type `unit` at
    /// an address, accessed volatile or not; signed fields extend their sign.
    Bits { pointer: i64, volatile: bool, start: i64, width: i64, unit: i64, signed: bool },
}

/// A block being built.
struct Open {
    id: i64,
    instructions: Vec<h::Instruction>,
    terminator: Option<h::Terminator>,
}

struct Body<'a, 't> {
    /// The facts the language states of instructions, by instruction id.
    stated_instructions: Vec<(i64, Fact)>,
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
    /// The far pointer to where a struct result goes, where it is passed one.
    destination: Option<i64>,
    /// `-g`: the unit's debug types, and the statement's source line.
    described: &'t mut Option<crate::debug::Described<'a>>,
    line: i64,
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

/// Parameters that live in the memory they were passed in, `sizes` each
/// parameter value's bytes. Borland's STDARG.H steps `va_start` on from the
/// last parameter's address by its size rounded to an int: in a variadic
/// function an address-taken parameter lives with the arguments after it.
/// An interrupt handler's parameters are the registers it saved, named in
/// Borland's order, each at its slot of the frame (`x86_intr_slot`); what it
/// writes to them is what it returns to.
fn in_their_slots(function: &mut h::Function, homes: &[(i64, i64)], sizes: &[i64], struct_homes: &[i64]) -> R<()> {
    let Some(abi) = function.abi.as_ref() else { return Ok(()) };
    let (interrupt, variadic) = (abi.distance == CallDistance::Interrupt, abi.variadic);
    if !interrupt && !variadic {
        return Ok(());
    }
    let exposed = llrm_core::hir::escape::exposed_frame(function);
    // A struct parameter is a copy of its words; its address is not theirs.
    if struct_homes.iter().any(|home| exposed.contains(home)) {
        return refuse(format!("{}: the address of a struct parameter where its arguments are addressed", function.name));
    }
    // STDARG.H's __size: rounded up to an int, the two bytes of Borland's.
    let rounded = |sizes: &[i64]| sizes.iter().map(|size| (size + 1) & !1).sum::<i64>();
    for &(home, parameter) in homes.iter().filter(|(home, _)| interrupt || exposed.contains(home)) {
        let at = function.parameters.iter().position(|&one| one == parameter).expect("a parameter");
        let offset = if interrupt { interrupt_slot(&function.name, at, sizes[at])? } else { -rounded(&sizes[at..]) };
        let place = function.places.iter_mut().find(|one| one.id == home).expect("a home");
        (place.storage, place.symbol, place.offset) = (Storage::Parameter, parameter, offset);
        let copy = |one: &h::Instruction| one.op == Op::Store && one.operands == [Operand::place_ref(home), value_ref(parameter)];
        for block in &mut function.blocks {
            block.instructions.retain(|one| !copy(one));
        }
    }
    Ok(())
}

/// Borland C's interrupt parameters, in order: the registers its handler saved.
const BORLAND_INTERRUPT_PARAMETERS: [&str; 12] = ["bp", "di", "si", "ds", "es", "dx", "cx", "bx", "ax", "ip", "cs", "flags"];

/// Where parameter `at` of handler `name`, `size` bytes, is in its frame: a
/// register is one word, the low word of its slot.
fn interrupt_slot(name: &str, at: usize, size: i64) -> R<i64> {
    let Some(register) = BORLAND_INTERRUPT_PARAMETERS.get(at) else { return refuse(format!("{name}: an interrupt handler's parameter {at}: Borland's has {}", BORLAND_INTERRUPT_PARAMETERS.len())) };
    if size != 2 {
        return refuse(format!("{name}: an interrupt handler's parameter {register} is {size} bytes, not a register"));
    }
    Ok(llrm_mir::opcode::x86_intr_slot(register).expect("every Borland register is in the frame"))
}

fn distance(symbol: &hir::Symbol) -> CallDistance {
    match symbol {
        _ if symbol.interrupt() => CallDistance::Interrupt,
        _ if symbol.far() => CallDistance::Far,
        _ => CallDistance::Near,
    }
}

/// How Borland C returns a struct of `size` bytes: an integer that wide in
/// AL, AX or DX:AX, or, where none, through a far pointer to the caller's
/// memory that it pushes after every argument and gets back in DX:AX.
fn returned_as_integer(size: i64) -> Option<i64> {
    matches!(size, 1 | 2 | 4).then_some(size)
}

/// A struct argument's words, by byte offset, in parameter order: they lie
/// as the struct does, the first lowest, so last first where arguments are
/// pushed in order.
fn struct_words(size: i64, in_order: bool) -> Vec<i64> {
    let words = (0..(size + 1) / 2).map(|at| at * 2);
    if in_order { words.rev().collect() } else { words.collect() }
}

/// `parameters` with the far pointer to a struct result where it goes:
/// pushed after every argument.
fn with_destination(parameters: &mut Vec<i64>, destination: i64, in_order: bool) {
    if in_order {
        parameters.push(destination);
    } else {
        parameters.insert(0, destination);
    }
}

impl<'a, 't> Body<'a, 't> {
    fn function(
        shared: &'a Shared<'a>,
        types: &'t mut Types<'a>,
        callables: &'t mut IndexMap<String, h::Callable>,
        described: &'t mut Option<crate::debug::Described<'a>>,
        facts: &mut Facts,
        proc: &'a hir::Proc,
        id: i64,
    ) -> R<h::Function> {
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
            stated_instructions: Vec::new(),
            calls: Vec::new(),
            instructions: 0,
            inlined: None,
            destination: None,
            described,
            line: 0,
        };
        let entry = body.block();
        body.current = entry;
        let result_type = match body.types.aggregate(&proc.type_) {
            _ if shared.valueless.contains(&symbol.id) => body.types.of(Shape::Void, None),
            Some(size) => match returned_as_integer(size) {
                Some(width) => body.types.raw(width),
                None => body.types.pointer(FAR),
            },
            None => body.ty(&proc.type_)?,
        };
        let (parameters, stated, homes, struct_homes) = body.frame()?;
        for one in &proc.body {
            body.statement(one)?;
        }
        body.finish()?;
        let mut body_instruction_facts = std::mem::take(&mut body.stated_instructions);
        let body_widths: HashMap<i64, i64> = parameters.iter().map(|&one| (one, body.types.get(body.values[one as usize - 1].r#type).width)).collect();
        let parameter_bytes = body_widths.values().sum();
        let blocks = body
            .blocks
            .into_iter()
            .map(|one| h::Block::new(one.id, one.instructions, one.terminator.expect("finished")))
            .collect();
        let linkage = if symbol.exported() { h::FunctionLinkage::External } else { h::FunctionLinkage::Internal };
        let mut function = h::Function {
            parameters,
            abi: Some(h::ProcedureAbi { cleanup, distance: distance(symbol), parameter_bytes, float_return: FloatReturn::Register, variadic: symbol.variadic() }),
            calls: body.calls,
            linkage,
            ..h::Function::new(id, &symbol.object_name(), result_type, body.values, body.places, blocks, 1)
        };
        for (instruction, fact) in std::mem::take(&mut body_instruction_facts) {
            facts.state(Subject::Instruction { function: id, id: instruction }, fact);
        }
        for (value, fact) in stated {
            let index = function.parameters.iter().position(|&one| one == value).expect("a parameter") as i64;
            facts.state(Subject::Param { function: id, index }, fact);
        }
        let sizes: Vec<i64> = function.parameters.iter().map(|&one| body_widths[&one]).collect();
        in_their_slots(&mut function, &homes, &sizes, &struct_homes)?;
        // Every address C computes through a pointer stays inside the object it points into.
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            for (index, operand) in instruction.operands.iter().enumerate() {
                if matches!(operand, h::Operand::IndirectPlace(_)) {
                    facts.state(Subject::Operand { function: id, instruction: instruction.id, operand: index as i64 }, Fact::InBounds);
                }
            }
        }
        if let Some(described) = body.described.as_mut() {
            // A parameter as its home: where the function keeps it.
            for &(symbol, handle) in &proc.debug {
                let name = &unit.symbols[&symbol].name;
                match (body.slots.get(&format!("y{symbol}")), shared.keys.get(&Key::Symbol(symbol))) {
                    (Some(&place), _) => described.variable(place, name, handle),
                    (None, Some(&object)) => described.local_static(object, name, handle),
                    (None, None) => {}
                }
            }
            described.function(id, &symbol.name, proc.debug_type);
        }
        Ok(function)
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
            Shape::Bool => Some(self.types.word() * 8),
            _ => None,
        }
    }

    /// A pointer's address space.
    fn space(&self, value: i64) -> Option<u32> {
        match self.shape(value) {
            Shape::Pointer(4) if !self.types.flat => Some(FAR),
            Shape::Pointer(_) => Some(0),
            Shape::Huge => Some(HUGE),
            _ => None,
        }
    }

    /// Whether two types are one MIR type.
    fn same(&self, a: i64, b: i64) -> bool {
        let mir = |shape| match shape {
            Shape::Int(width, _) => Shape::Int(width, false),
            Shape::Bool => Shape::Int(self.types.word(), false),
            other => other,
        };
        mir(self.types.shape(a)) == mir(self.types.shape(b))
    }

    fn instruction(&mut self, op: Op, results: Vec<i64>, operands: Vec<Operand>) -> &mut h::Instruction {
        self.instructions += 1;
        let mut made = h::Instruction::new(self.instructions, op, results, operands);
        if self.described.is_some() && self.line > 0 {
            made.line = Some(self.line);
        }
        let block = &mut self.blocks[self.current];
        block.instructions.push(made);
        block.instructions.last_mut().expect("just pushed")
    }

    /// That C promises the result of instruction `id` fits its type.
    fn state_no_wrap(&mut self, id: i64, promised: bool) {
        if promised {
            self.stated_instructions.push((id, Fact::NoSignedWrap));
        }
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
    /// `value` as a place of type `ty` holds it: an integer of the same
    /// width but another signedness or class is converted to `ty`.
    fn fitted(&mut self, value: i64, ty: i64) -> i64 {
        let had = self.type_of(value);
        if had == ty {
            return value;
        }
        match (self.types.shape(had), self.types.shape(ty)) {
            (Shape::Int(from, _), Shape::Int(to, _)) if from == to => self.op(Op::Convert, ty, vec![value_ref(value)]),
            (Shape::Bool, Shape::Int(..)) => self.op(Op::Convert, ty, vec![value_ref(value)]),
            (Shape::Pointer(from), Shape::Pointer(to)) if from == to => self.op(Op::Copy, ty, vec![value_ref(value)]),
            _ => value,
        }
    }

    fn as_signed(&mut self, value: i64, is_signed: bool) -> i64 {
        match self.shape(value) {
            Shape::Int(_, now) if now == is_signed => value,
            Shape::Int(width, _) => {
                let ty = self.types.int(width, is_signed);
                self.op(Op::Convert, ty, vec![value_ref(value)])
            }
            Shape::Bool => {
                let ty = self.types.int(2, is_signed);
                self.op(Op::Convert, ty, vec![value_ref(value)])
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
    /// A scalar parameter's home is also returned, by place and parameter.
    fn frame(&mut self) -> R<(Vec<i64>, Vec<(i64, Fact)>, Vec<(i64, i64)>, Vec<i64>)> {
        let mut parameters = Vec::new();
        let mut stated = Vec::new();
        let mut homes = Vec::new();
        let mut struct_homes = Vec::new();
        let mut stores = Vec::new();
        let in_order = self.unit.symbols[&self.proc.symbol].in_order();
        for (symbol, type_) in self.proc.parameters(&self.unit.symbols[&self.proc.symbol]) {
            match self.types.aggregate(type_) {
                Some(size) => {
                    let words = (size + 1) / 2;
                    let ty = self.types.of(Shape::Bytes(words * 2), None);
                    let place = self.local(&format!("y{symbol}"), ty, words * 2);
                    let word = self.types.raw(2);
                    struct_homes.push(place);
                    for at in struct_words(size, in_order) {
                        let parameter = self.value(word);
                        parameters.push(parameter);
                        stores.push((place, at, parameter));
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
                    homes.push((place, parameter));
                    self.slots.insert(format!("y{symbol}"), place);
                    // What the language states of a parameter: C99 6.7.3.1 for restrict.
                    stated.extend(crate::ow_facts::of_param(self.unit, *symbol).into_iter().map(|fact| (parameter, fact)));
                }
            }
        }
        // A struct result goes where a far pointer pushed after every argument says.
        if self.types.aggregate(&self.proc.type_).is_some_and(|size| returned_as_integer(size).is_none()) {
            let far = self.types.pointer(FAR);
            let destination = self.value(far);
            with_destination(&mut parameters, destination, in_order);
            self.destination = Some(destination);
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
        Ok((parameters, stated, homes, struct_homes))
    }

    // ---- statements ----

    fn statement(&mut self, one: &hir::Statement) -> R<()> {
        self.line = one.line;
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
        if let Some(size) = self.types.aggregate(type_) {
            let Got::Aggregate(from, _) = self.eval(node)? else { return self.refuse("a scalar returned as a struct") };
            let value = match (returned_as_integer(size), self.destination) {
                (Some(width), _) => {
                    let ty = self.types.raw(width);
                    self.op(Op::Load, ty, vec![Operand::IndirectPlace(indirect(from, 0, ty, false))])
                }
                (None, Some(destination)) => {
                    self.copy(destination, from, size);
                    destination
                }
                (None, None) => return self.refuse("a struct result with no destination"),
            };
            self.terminate(TerminatorKind::Return, vec![value_ref(value)], Vec::new());
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
            Shape::Pointer(4) if !self.types.flat => {
                let dword = self.types.int(4, false);
                self.op(Op::Convert, dword, vec![value_ref(value)])
            }
            Shape::Pointer(_) | Shape::Huge => {
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
        // Borland orders far pointers by their offsets alone and compares
        // them equal by all 32 bits. Near and huge pointers compare as
        // pointers: how a huge pointer orders is isel's (its packed bits
        // are no address, so no conversion to read).
        let equality = matches!(cg_op.as_str(), "O_EQ" | "O_NE");
        let (a, b) = match self.space(a) {
            Some(FAR) => {
                let (op, width) = if equality { (Op::Convert, 4) } else { (Op::PointerOffset, 2) };
                let part = self.types.int(width, false);
                (self.op(op, part, vec![value_ref(a)]), self.op(op, part, vec![value_ref(b)]))
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
            "CGFact" => self.type_of_node(&tree.args[1]),
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
            Got::Bits { .. } => self.refuse("a bit field's address"),
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
                    Shape::Pointer(_) | Shape::Huge => {
                        let width = self.types.get(ty).width;
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
                if let Got::Bits { signed, .. } = got {
                    let stored = self.write_bits(got, value)?;
                    let ty = self.ty(type_)?;
                    return Ok(Got::Value(self.resized(stored, signed, ty)));
                }
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
                if let Got::Bits { signed, .. } = got {
                    let ty = self.ty(type_)?;
                    let read = self.read_bits(got)?;
                    let old = self.resized(read, signed, ty);
                    let by = self.value_as(source, type_)?;
                    let new = self.arithmetic(cg_op, old, by, type_)?;
                    let stored = self.write_bits(got, new)?;
                    let stored = self.resized(stored, signed, ty);
                    return Ok(Got::Value(if call == "CGPostGets" { old } else { stored }));
                }
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
            ("CGEval", [inner]) | ("CGAttr", [inner, _]) | ("CGFact", [_, inner, _]) => self.eval(inner)?,
            ("CGVolatile", [inner]) => {
                let got = self.eval(inner)?;
                Got::Volatile(self.address(got)?.0)
            }
            ("CGBitMask", [inner, start, width, type_]) => {
                let got = self.eval(inner)?;
                let (pointer, volatile) = self.address(got)?;
                let number = |text: &str| text.parse::<i64>().map_err(|_| hir::Unsupported(format!("a bit field's {text}")));
                Got::Bits { pointer, volatile, start: number(start)?, width: number(width)?, unit: self.ty(type_)?, signed: signed(&self.unit.canonical_type(type_)) }
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

    /// A based pointer, `segment :> offset` (Watcom's binary O_CONVERT): the
    /// far pointer of `offset` in `segment`, which is a segment's symbol (as
    /// `__segname` names one), a far pointer whose selector it takes (a
    /// function's, for `_CODE`), or a selector's value.
    fn based(&mut self, offset: &str, segment: &str) -> R<i64> {
        let near = self.value_as(offset, "TY_NEAR_POINTER")?;
        let far = self.types.pointer(FAR);
        let dword = self.types.int(4, false);
        let selector = match self.segment_symbol(segment) {
            // DGROUP's: the near pointer made far, as any near pointer is.
            Some(symbol) if self.unit.grouped(symbol) => return Ok(self.op(Op::Convert, far, vec![value_ref(near)])),
            // Another segment's selector: its symbol is an empty object placed in it.
            Some(symbol) => {
                let (name, key) = (symbol.name.clone(), Key::Symbol(symbol.id));
                let Some(&object) = self.shared.keys.get(&key) else { return self.refuse(format!("segment {name} is no object here")) };
                let place = self.global(object);
                let pointer = self.address_of(place);
                let whole = self.op(Op::Convert, dword, vec![value_ref(pointer)]);
                let high = self.constant(dword, Number::Int(0xFFFF_0000u32.into()));
                self.op(Op::And, dword, vec![value_ref(whole), value_ref(high)])
            }
            None => {
                let got = self.eval(segment)?;
                let value = self.scalar(got)?;
                match self.space(value) {
                    Some(FAR | HUGE) => {
                        let whole = self.op(Op::Convert, dword, vec![value_ref(value)]);
                        let high = self.constant(dword, Number::Int(0xFFFF_0000u32.into()));
                        self.op(Op::And, dword, vec![value_ref(whole), value_ref(high)])
                    }
                    Some(_) => return self.refuse("a near pointer as a based pointer's segment"),
                    None => {
                        let wide = self.resized(value, false, dword);
                        let sixteen = self.constant(dword, Number::Int(16.into()));
                        self.op(Op::Shl, dword, vec![value_ref(wide), value_ref(sixteen)])
                    }
                }
            }
        };
        let word = self.types.int(2, false);
        let low = self.op(Op::Convert, word, vec![value_ref(near)]);
        let low = self.resized(low, false, dword);
        let joined = self.op(Op::Or, dword, vec![value_ref(selector), value_ref(low)]);
        Ok(self.op(Op::Convert, far, vec![value_ref(joined)]))
    }

    /// The segment a `__segname` names: its `.NAME` symbol.
    fn segment_symbol(&self, node: &str) -> Option<&hir::Symbol> {
        let tree = self.unit.nodes.get(&hir::handle(node))?;
        let ("CGFEName", [symbol, _]) = (tree.call.as_str(), tree.args.as_slice()) else { return None };
        self.unit.symbols.get(&hir::handle(symbol)).filter(|one| one.name.starts_with('.') && !one.proc())
    }

    /// A bit field's value, in its unit's type.
    fn read_bits(&mut self, got: Got) -> R<i64> {
        let Got::Bits { pointer, volatile, start, width, unit, signed } = got else { return self.refuse("a bit field read of a non-field") };
        let bits = self.types.get(unit).width * 8;
        let whole = self.op(Op::Load, unit, vec![Operand::IndirectPlace(indirect(pointer, 0, unit, volatile))]);
        Ok(if signed {
            let (up, down) = (self.constant(unit, Number::Int((bits - start - width).into())), self.constant(unit, Number::Int((bits - width).into())));
            let raised = self.op(Op::Shl, unit, vec![value_ref(whole), value_ref(up)]);
            self.op(Op::Sar, unit, vec![value_ref(raised), value_ref(down)])
        } else {
            let (shift, mask) = (self.constant(unit, Number::Int(start.into())), self.constant(unit, Number::Int(((1i64 << width) - 1).into())));
            let lowered = self.op(Op::Shr, unit, vec![value_ref(whole), value_ref(shift)]);
            self.op(Op::And, unit, vec![value_ref(lowered), value_ref(mask)])
        })
    }

    /// `value` stored in a bit field, the unit's other bits kept: what the
    /// field then reads, in its unit's type.
    fn write_bits(&mut self, got: Got, value: i64) -> R<i64> {
        let Got::Bits { pointer, volatile, start, width, unit, signed } = got else { return self.refuse("a bit field write to a non-field") };
        let bits = self.types.get(unit).width * 8;
        let field = ((1i64 << width) - 1) << start;
        let kept = !field & ((1i64 << bits) - 1);
        let value = self.resized(value, signed, unit);
        let place = || Operand::IndirectPlace(indirect(pointer, 0, unit, volatile));
        let whole = self.op(Op::Load, unit, vec![place()]);
        let (shift, inside, outside) = (
            self.constant(unit, Number::Int(start.into())),
            self.constant(unit, Number::Int(field.into())),
            self.constant(unit, Number::Int(kept.into())),
        );
        let placed = self.op(Op::Shl, unit, vec![value_ref(value), value_ref(shift)]);
        let part = self.op(Op::And, unit, vec![value_ref(placed), value_ref(inside)]);
        let rest = self.op(Op::And, unit, vec![value_ref(whole), value_ref(outside)]);
        let joined = self.op(Op::Or, unit, vec![value_ref(rest), value_ref(part)]);
        self.instruction(Op::Store, Vec::new(), vec![place(), value_ref(joined)]);
        Ok(if signed {
            let (up, down) = (self.constant(unit, Number::Int((bits - start - width).into())), self.constant(unit, Number::Int((bits - width).into())));
            let raised = self.op(Op::Shl, unit, vec![value_ref(part), value_ref(up)]);
            self.op(Op::Sar, unit, vec![value_ref(raised), value_ref(down)])
        } else {
            let lowered = self.op(Op::Shr, unit, vec![value_ref(part), value_ref(shift)]);
            let mask = self.constant(unit, Number::Int(((1i64 << width) - 1).into()));
            self.op(Op::And, unit, vec![value_ref(lowered), value_ref(mask)])
        })
    }

    /// O_POINTS: what `got` addresses, read as `type_`.
    fn points(&mut self, got: Got, type_: &str) -> R<Got> {
        if let Got::Bits { signed, .. } = got {
            let read = self.read_bits(got)?;
            let ty = self.ty(type_)?;
            return Ok(Got::Value(self.resized(read, signed, ty)));
        }
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
        let value = self.fitted(value, ty);
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
        let at = self.int(self.types.word(), by);
        let result = self.value(self.type_of(pointer));
        let id = self.instruction(Op::PtrOffset, vec![result], vec![value_ref(pointer), value_ref(at)]).id;
        self.stated_instructions.push((id, Fact::InBounds));
        result
    }

    /// `value`, of C type `from`, as `to`.
    fn converted(&mut self, value: i64, from: &str, to: &str) -> R<i64> {
        let ty = self.ty(to)?;
        let changed = self.changed(value, from, ty)?;
        Ok(self.fitted(changed, ty))
    }

    /// `converted`, but for the type it may leave in another class or signedness.
    fn changed(&mut self, value: i64, from: &str, ty: i64) -> R<i64> {
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
            (Shape::Huge, Shape::Huge) => value,
            (Shape::Pointer(_) | Shape::Huge, Shape::Pointer(_) | Shape::Huge) => self.op(Op::Convert, ty, vec![value_ref(value)]),
            // An integer is the pointer's own bits, extended to its width as
            // Borland does: a far one is segment:offset, 0 the null pointer.
            (Shape::Int(..) | Shape::Bool, Shape::Pointer(_) | Shape::Huge) => {
                let width = self.types.get(ty).width;
                let int = self.types.int(width, false);
                let value = self.resized(value, source_signed, int);
                self.op(Op::Convert, ty, vec![value_ref(value)])
            }
            (Shape::Pointer(_) | Shape::Huge, Shape::Int(..)) => {
                let width = self.types.get(self.type_of(value)).width;
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
                let id = self.instruction(Op::Neg, vec![result], vec![value_ref(value)]).id;
                self.state_no_wrap(id, nowrap);
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

    /// The index of a pointer add scaled to bytes, `index * size`: the
    /// compiler's multiply, not an `int` product, so C promises nothing signed
    /// of it (`a[i]` with `i` past 16383 of a far word array is valid where the
    /// byte offset passes 32767). An unsigned index promises the offset fits the
    /// 64K segment an object lives in: the product does not wrap unsigned.
    /// Stated here, where the scale is made, from the index's C type.
    fn scaled(&mut self, node: &str) -> R<()> {
        let tree = self.unit.nodes[&hir::handle(node)].clone();
        let [op, left, right, type_] = &tree.args[..] else { return Ok(()) };
        if tree.call != "CGBinary" || op != "O_TIMES" || is_float(&self.unit.canonical_type(type_)) {
            return Ok(());
        }
        let constant = |this: &Self, node: &String| this.unit.nodes[&hir::handle(node)].call == "CGInteger";
        let (index, size) = match (constant(self, left), constant(self, right)) {
            (false, true) => (left, right),
            (true, false) => (right, left),
            _ => return Ok(()),
        };
        let unsigned = !signed(&self.unit.canonical_type(&self.type_of_node(index)));
        let (a, b) = (self.value_as(index, type_)?, self.value_as(size, type_)?);
        let ty = self.ty(type_)?;
        let result = self.value(ty);
        let (a, b) = if index == left { (a, b) } else { (b, a) };
        let id = self.instruction(Op::Mul, vec![result], vec![value_ref(a), value_ref(b)]).id;
        if unsigned {
            self.stated_instructions.push((id, Fact::NoUnsignedWrap));
        }
        self.done.insert(hir::handle(node), Got::Value(result));
        Ok(())
    }

    /// The value of integer constant `node`, unwrapped to its C type.
    fn whole_constant(&self, node: &str) -> Option<i64> {
        let tree = self.unit.nodes.get(&hir::handle(node))?;
        let ("CGInteger", [value, _]) = (tree.call.as_str(), &tree.args[..]) else { return None };
        value.trim().parse::<i64>().ok().filter(|bytes| i32::try_from(*bytes).is_ok())
    }

    fn binary(&mut self, cg_op: &str, left: &str, right: &str, type_: &str) -> R<i64> {
        if cg_op == "O_CONVERT" {
            return self.based(left, right);
        }
        let canonical = self.unit.canonical_type(type_);
        if matches!(cg_op, "O_PLUS" | "O_MINUS") && !is_float(&canonical) {
            let a_got = self.eval(left)?;
            if pointers(&canonical) {
                self.scaled(right)?;
            }
            let huge = canonical == "TY_HUGE_POINTER";
            // Watcom states a folded displacement past 16 bits as a TY_INTEGER constant
            // whose value is whole: a huge pointer takes all of it.
            let whole = if huge { self.whole_constant(right) } else { None };
            let b_got = match whole {
                Some(bytes) => {
                    let dword = self.types.int(4, true);
                    Got::Value(self.constant(dword, Number::Int(bytes)))
                }
                None => self.eval(right)?,
            };
            let (a, b) = (self.scalar(a_got)?, self.scalar(b_got)?);
            // Pointer arithmetic: the pointer moved, or two pointers' distance.
            match (self.space(a).is_some(), self.space(b).is_some()) {
                // The pointer is made huge first: a far one moves inside its segment.
                (true, false) if huge => {
                    let a = self.converted(a, &self.type_of_node(left), type_)?;
                    let from = self.type_of_node(right);
                    return self.moved(a, b, &from, cg_op == "O_MINUS");
                }
                (false, true) if huge && cg_op == "O_PLUS" => {
                    let b = self.converted(b, &self.type_of_node(right), type_)?;
                    let from = self.type_of_node(left);
                    return self.moved(b, a, &from, false);
                }
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
                // Two huge pointers' distance: their bytes, not their packed bits.
                (true, true) if cg_op == "O_MINUS" && !pointers(&canonical) && self.space(a) == Some(HUGE) && self.space(b) == Some(HUGE) => {
                    let ty = self.ty(type_)?;
                    if self.types.get(ty).width != 4 {
                        return self.refuse(format!("a huge pointer difference as {type_}"));
                    }
                    return Ok(self.op(Op::PtrDiff, ty, vec![value_ref(a), value_ref(b)]));
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
        // A huge pointer's index is a dword; a far one's its offset's word.
        let word = self.types.int(if self.space(pointer) == Some(HUGE) { 4 } else { self.types.word() }, true);
        let by = match self.bits(by) {
            Some(_) => self.resized(by, signed(&self.unit.canonical_type(from)), word),
            None => return self.refuse(format!("a pointer moved by a {from}")),
        };
        let by = if subtract { self.op(Op::Neg, word, vec![value_ref(by)]) } else { by };
        let ty = self.type_of(pointer);
        let result = self.value(ty);
        let id = self.instruction(Op::PtrOffset, vec![result], vec![value_ref(pointer), value_ref(by)]).id;
        self.stated_instructions.push((id, Fact::InBounds));
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
        let id = self.instruction(op, vec![result], vec![value_ref(a), value_ref(b)]).id;
        self.state_no_wrap(id, nowrap);
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
        self.calls.push(h::CallAbi { instruction: id, order, cleanup, distance, callee: None, float_return: FloatReturn::Register });
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
            let Got::Aggregate(from, _) = self.eval(node)? else { return self.refuse("a scalar passed as an aggregate") };
            // Listed last first, as the arguments are; a last odd byte widened.
            let word = self.types.raw(2);
            for at in struct_words(size, symbol.in_order()).into_iter().rev() {
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
        // A struct result: in a temporary, which the callee fills through a
        // far pointer pushed last where it is not returned as an integer.
        let returned = self.types.aggregate(&call.type_).map(|size| {
            let ty = self.types.of(Shape::Bytes(size), None);
            (self.local(&format!("r{}", self.places.len() + 1), ty, size), size)
        });
        if let Some((temporary, _)) = returned.filter(|&(_, size)| returned_as_integer(size).is_none()) {
            let near = self.address_of(temporary);
            let far = self.types.pointer(FAR);
            let destination = self.op(Op::Convert, far, vec![value_ref(near)]);
            with_destination(&mut arguments, destination, symbol.in_order());
        }
        let result = match (target, returned) {
            (Got::Function(symbol), _) if self.shared.valueless.contains(&symbol) => None,
            (_, Some((_, size))) => {
                let ty = match returned_as_integer(size) {
                    Some(width) => self.types.raw(width),
                    None => self.types.pointer(FAR),
                };
                Some(self.value(ty))
            }
            (_, None) => {
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
        let Some((temporary, size)) = returned else { return Ok(Got::Returned(result)) };
        let address = self.address_of(temporary);
        if let (Some(width), Some(value)) = (returned_as_integer(size), result) {
            let ty = self.types.raw(width);
            let value = self.fitted(value, ty);
            self.instruction(Op::Store, Vec::new(), vec![Operand::IndirectPlace(indirect(address, 0, ty, false)), value_ref(value)]);
        }
        Ok(Got::Value(address))
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
            returns_twice: false,
            symbol: None,
        })
        .id
}

/// An access through `base`, `offset` bytes in, as `ty`.
fn indirect(base: i64, offset: i64, ty: i64, volatile: bool) -> h::IndirectPlace {
    h::IndirectPlace { base, offset, r#type: ty, volatile, origin: None, allocation: None, member: None }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use llrm_mir::{Attribute, Linkage, Module};

    use crate::{hir, stream};

    fn raised(fixture: &str) -> Module {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c").join(fixture)).unwrap();
        let program = super::program(&hir::unit(&stream::parse(&text)).unwrap(), "test").unwrap();
        let emitted = llrm_core::hir::mir::emit(&program, &llrm_x86_m16::layout()).swap_remove(0);
        assert_eq!(emitted.refused, Vec::<(String, String)>::new());
        emitted.module
    }

    /// The C library's strcmp is stated a three-way compare: branchprob
    /// read `strcmp(a, b) > 0` as a likely `x > 0`.
    #[test]
    fn strcmp_is_stated_a_three_way_compare() {
        let module = raised("threeway.cgs");
        let callee = module.globals.iter().find(|one| one.name.as_deref() == Some("_strcmp")).and_then(|one| one.function()).expect("strcmp declared");
        assert!(llrm_mir::facts::Facts::of(&callee.attrs).three_way_compare(), "{}", llrm_mir::print::module(&module));
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

    /// The attributes of the function `@name`, declared or defined.
    fn attributes(module: &Module, name: &str) -> Vec<llrm_mir::Attribute> {
        module.global(module.named(name).unwrap()).function().unwrap().attrs.clone()
    }

    /// `__declspec(noreturn)` and `#pragma aux ... aborts` are in the call
    /// class; the compile dropped them, and the call fell through to the
    /// code after it.
    #[test]
    fn test_noreturn_and_aborts_are_stated_of_the_callee() {
        let module = raised("tests/test_noreturn_and_aborts_are_stated_of_the_callee.cgs");
        let noreturn = llrm_mir::facts::Fact::NoReturn.carrier();
        assert!(attributes(&module, "_die").contains(&noreturn));
        assert!(attributes(&module, "_quit").contains(&noreturn));
        assert!(!attributes(&module, "_f").contains(&noreturn));
    }

    /// `#pragma aux ... parm nomemory modify nomemory` is a routine that
    /// touches no memory; the compile dropped it and kept both calls.
    #[test]
    fn test_nomemory_is_stated_of_the_callee() {
        let module = raised("tests/test_nomemory_is_stated_of_the_callee.cgs");
        assert_eq!(attributes(&module, "_sq"), vec![llrm_mir::Attribute::Memory(vec![(None, "none".to_owned())])]);
    }

    /// The facts C states of `add` hold when it runs: called on three arrays it
    /// runs, and called with one array as two of its restrict parameters it is
    /// the caller that broke restrict, which the checked interpreter reports.
    #[test]
    fn test_a_restrict_call_on_one_array_is_reported_by_the_checked_run() {
        use llrm_mir::interpret::{Trap, run_checked};
        let text = format!(
            "{}\n@one = global [8 x i16] zeroinitializer\n@two = global [8 x i16] zeroinitializer\n@three = global [8 x i16] zeroinitializer\n\
             define void @apart() {{\n  call addrspace(1) void @_add(ptr @one, ptr @two, ptr @three)\n  ret void\n}}\n\
             define void @same() {{\n  call addrspace(1) void @_add(ptr @one, ptr @one, ptr @three)\n  ret void\n}}\n",
            llrm_mir::print::module(&raised("tests/test_restrict_reaches_mir_as_distinct_noalias_roots.cgs"))
        );
        let module = llrm_mir::parse::module(&text).unwrap_or_else(|error| panic!("{error}\n{text}"));
        assert!(run_checked(&module, "apart", Vec::new(), 10_000).is_ok(), "{:?}", run_checked(&module, "apart", Vec::new(), 10_000));
        let trapped = run_checked(&module, "same", Vec::new(), 10_000).unwrap_err();
        assert!(matches!(&trapped, Trap::Undefined(why) if why.contains("noalias parameter")), "{trapped:?}");
    }

    /// `__based` pointers were refused (the binary O_CONVERT of a segment and
    /// an offset). One in DGROUP reads its object; one in the code segment or
    /// in a segment holding an object of the unit is that segment's far pointer.
    #[test]
    fn test_based_pointers_are_far_pointers_into_their_segment() {
        let module = raised("based.cgs");
        let read = llrm_mir::interpret::run(&module, "_read_data", Vec::new(), 10_000).unwrap_or_else(|trap| panic!("{trap:?}"));
        assert_eq!(read, llrm_mir::interpret::Val::Int { bits: 42, width: 16 });
    }

    /// A segment the unit places nothing in still has a selector: its
    /// `__segname` symbol, an empty object in that segment, whose far
    /// address the based pointer takes its selector from.
    #[test]
    fn test_a_based_pointer_takes_its_segments_selector() {
        let module = raised("basednone.cgs");
        let text = defined(&module, "_read_elsewhere");
        assert!(text.contains("ptrtoint ptr addrspace(1) @_.ELSEWHERE"), "{text}");
    }

    /// `fixture`'s HIR program.
    fn program_of(fixture: &str) -> llrm_core::hir::model::Program {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c").join(fixture)).unwrap();
        super::program(&hir::unit(&stream::parse(&text)).unwrap(), "test").unwrap()
    }

    /// llrm-c's HIR had never met the verifier, and failed it on 22 of the
    /// corpus's programs (#224): a value of one signedness or pointer class
    /// stored or compared as another, near and huge pointers ordered
    /// unconverted, and C's untyped pointers and indirect calls.
    #[test]
    fn test_every_c_fixture_is_valid_hir() {
        let root = Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c");
        let mut fixtures: Vec<String> = Vec::new();
        for directory in ["", "tests", "parity"] {
            let Ok(read) = std::fs::read_dir(root.join(directory)) else { continue };
            for entry in read.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.ends_with(".cgs") {
                    fixtures.push(if directory.is_empty() { name } else { format!("{directory}/{name}") });
                }
            }
        }
        assert!(fixtures.len() > 50, "premise: the fixtures are found: {fixtures:?}");
        let invalid: Vec<String> = fixtures
            .iter()
            .filter_map(|fixture| {
                let text = std::fs::read_to_string(root.join(fixture)).unwrap();
                let program = super::program(&hir::unit(&stream::parse(&text)).ok()?, "test").ok()?;
                llrm_core::hir::verify::verify(&program).err().map(|why| format!("{fixture}: {why:?}"))
            })
            .collect();
        assert!(invalid.is_empty(), "{invalid:#?}");
    }

    /// The driver checked only dominance (#224); it now refuses any program
    /// the HIR verifier refuses, whichever frontend made it.
    #[test]
    fn test_the_driver_refuses_what_the_verifier_refuses() {
        let mut program = program_of("bytes.cgs");
        let machine = llrm_core::abi::machine::BUILT_IN.clone();
        let options = llrm_driver::m16_options(machine);
        assert!(llrm_core::driver::emitted(&program, &options).is_ok(), "premise: valid as raised");
        let module = &mut program.modules[0];
        let byte = module.types.iter().find(|one| one.kind == llrm_core::hir::model::TypeKind::Integer && one.width == 1).map(|one| one.id).expect("a byte type");
        let stored = module.functions.iter_mut().flat_map(|one| &mut one.blocks).flat_map(|one| &mut one.instructions).find(|one| {
            one.op == llrm_core::hir::model::Op::Store && matches!(&one.operands[0], llrm_core::hir::model::Operand::IndirectPlace(place) if place.r#type != byte)
        });
        let Some(llrm_core::hir::model::Operand::IndirectPlace(place)) = stored.map(|one| &mut one.operands[0]) else { panic!("premise: a store through a pointer") };
        place.r#type = byte;
        let why = llrm_core::driver::emitted(&program, &options).err().unwrap_or_default();
        assert!(why.contains("store value type does not match its place"), "{why}");
    }

    /// The verifier lets C's untyped pointers through, but a pointer that
    /// states its pointee still bounds what is read through it.
    #[test]
    fn test_a_stated_pointee_still_bounds_an_indirect_place() {
        let mut program = program_of("bytes.cgs");
        assert!(llrm_core::hir::verify::verify(&program).is_ok(), "premise: valid as raised");
        let module = &mut program.modules[0];
        let byte = module.types.iter().find(|one| one.kind == llrm_core::hir::model::TypeKind::Integer && one.width == 1).map(|one| one.id).expect("a byte type");
        let wider = |ty: i64| module.types.iter().find(|one| one.id == ty).is_some_and(|one| one.width > 1);
        let base = module.functions.iter().flat_map(|one| &one.blocks).flat_map(|one| &one.instructions).flat_map(|one| &one.operands).find_map(|operand| match operand {
            llrm_core::hir::model::Operand::IndirectPlace(place) if wider(place.r#type) => Some(place.base),
            _ => None,
        }).expect("premise: a place wider than a byte, read through a pointer");
        let pointer = module.functions.iter().flat_map(|one| &one.values).find(|one| one.id == base).map(|one| one.r#type).unwrap();
        let narrowed = module.types.iter().map(|one| one.id).max().unwrap() + 1;
        let mut typed = module.types.iter().find(|one| one.id == pointer).unwrap().clone();
        (typed.id, typed.element) = (narrowed, Some(byte));
        module.types.push(typed);
        for function in &mut module.functions {
            for value in function.values.iter_mut().filter(|one| one.id == base) {
                value.r#type = narrowed;
            }
        }
        let why = llrm_core::hir::verify::verify(&program).unwrap_err();
        assert!(format!("{why:?}").contains("exceeds its pointee"), "{why:?}");
    }

    /// An indirect call passes the operands after the pointer it calls
    /// through; its order is over those, and a gap in it is still refused.
    #[test]
    fn test_an_indirect_calls_order_is_over_its_arguments() {
        let mut program = program_of("codeptrs.cgs");
        assert!(llrm_core::hir::verify::verify(&program).is_ok(), "{:?}", llrm_core::hir::verify::verify(&program));
        let function = program.modules[0].functions.iter_mut().find(|one| one.calls.iter().any(|site| !site.order.is_empty())).unwrap();
        let site = function.calls.iter_mut().find(|site| !site.order.is_empty()).unwrap();
        site.order.iter_mut().for_each(|one| *one += 1);
        let why = llrm_core::hir::verify::verify(&program).unwrap_err();
        assert!(format!("{why:?}").contains("invalid argument order"), "{why:?}");
    }

    /// Bit fields were refused ("CGBitMask"). Written through its fields, the
    /// struct holds Borland's bytes, and each field reads back, signed ones
    /// with their sign, through `++`, `--` and an assignment's value.
    #[test]
    fn test_bit_fields_read_and_write_borlands_layout() {
        let module = raised("bitfield.cgs");
        let failed = llrm_mir::interpret::run(&module, "_check", Vec::new(), 100_000).unwrap_or_else(|trap| panic!("{trap:?}"));
        assert_eq!(failed, llrm_mir::interpret::Val::Int { bits: 0, width: 16 }, "failed checks: {failed:?}");
    }

    /// Borland orders far pointers by their offsets (bcc -S: `cmp ax,
    /// [bp+10]` then `jae`) and compares them equal by all 32 bits. llrm-c
    /// ordered all 32 bits, so 2000:0010 was not below 1000:0020.
    #[test]
    fn test_a_far_pointer_orders_by_its_offset() {
        use llrm_mir::interpret::{Val, run};
        let module = raised("farorder.cgs");
        let call = |function: &str, a: u64, b: u64| run(&module, function, vec![Val::Ptr(a), Val::Ptr(b)], 1_000).unwrap_or_else(|trap| panic!("{trap:?}"));
        let (yes, no) = (Val::Int { bits: 1, width: 16 }, Val::Int { bits: 0, width: 16 });
        assert_eq!(call("_below", 0x2000_0010, 0x1000_0020), yes);
        assert_eq!(call("_below", 0x1000_0020, 0x2000_0010), no);
        assert_eq!(call("_same", 0x2000_0010, 0x1000_0010), no);
        assert_eq!(call("_same", 0x2000_0010, 0x2000_0010), yes);
    }

    /// C99 6.7.3.1: the three restrict parameters of `add` reach distinct objects.
    #[test]
    fn test_restrict_parameters_are_noalias() {
        let module = raised("tests/test_restrict_reaches_mir_as_distinct_noalias_roots.cgs");
        let function = module.global(module.named("_add").unwrap()).function().unwrap();
        assert!((0..function.parameter_attrs.len()).all(|at| llrm_mir::facts::Facts::param(function, at).no_alias()), "{:?}", function.parameter_attrs);
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

    /// `a[i]` with an unsigned `i` was scaled by `mul nsw i16 %i, 2`, poison
    /// for i >= 16384 though the access of a far word array is valid (#100).
    /// C promises nothing signed of the multiply that scales an index to bytes; of
    /// an unsigned index, that it does not wrap unsigned.
    #[test]
    fn test_the_multiply_scaling_an_index_states_no_wrap() {
        let module = raised("unsignedindex.cgs");
        let text = defined(&module, "_sum");
        let scales: Vec<&str> = text.lines().filter(|one| one.contains(" = mul ")).collect();
        assert!(!scales.is_empty(), "the shape that was stated nsw: {text}");
        assert!(scales.iter().all(|one| !one.contains("nsw") && one.contains("mul nuw i16")), "{scales:#?}");
    }

    /// The scaling multiply's flag follows the index's C type, however the index
    /// arrives: an unsigned one through a conversion (`(unsigned)c`) or a sum
    /// (`u + 1`) is `nuw`; a signed one carries none (#150).
    #[test]
    fn test_the_scale_of_an_index_follows_the_indexs_type() {
        let module = raised("scaledindex.cgs");
        let scale = |name: &str| {
            let text = defined(&module, name);
            let muls: Vec<String> = text.lines().filter(|one| one.contains(" = mul ")).map(str::to_owned).collect();
            assert_eq!(muls.len(), 1, "{name}: {text}");
            muls[0].clone()
        };
        assert!(scale("_through_a_char").contains("mul nuw i16"), "{}", scale("_through_a_char"));
        assert!(scale("_through_a_sum").contains("mul nuw i16"), "{}", scale("_through_a_sum"));
        assert!(!scale("_through_a_signed").contains("nsw") && !scale("_through_a_signed").contains("nuw"), "{}", scale("_through_a_signed"));
    }

    /// Every conversion to or from a long double was E1090 in the front end
    /// (#103), and mixed arithmetic left its operand unconverted: the MIR
    /// converts each way, between x86_fp80 and float, double and the integers.
    #[test]
    fn test_long_double_converts_to_and_from_every_arithmetic_type() {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c/longdouble.cgs")).unwrap();
        assert!(text.contains("O_CONVERT") && text.contains("TY_LONG_DOUBLE"), "the shape the front end refused");
        let module = raised("longdouble.cgs");
        let printed = llrm_mir::print::module(&module);
        for conversion in ["fptrunc x86_fp80", "fpext double", "fptosi x86_fp80", "sitofp i16"] {
            assert!(printed.contains(conversion), "{conversion}: {printed}");
        }
        let main = defined(&module, "_main");
        assert!(main.contains("sitofp") && main.contains("x86_fp80"), "{main}");
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
