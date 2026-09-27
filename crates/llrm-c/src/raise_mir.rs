//! One unit's trees as the rich MIR (llrm-mir), as clang's CodeGen emits
//! LLVM IR: each C object an alloca or a global, each tree node its
//! instructions, promotion left to mem2reg.
//!
//! What C promises goes into the IR: `!tbaa` by its aliasing classes,
//! `noalias` on a restrict parameter, internal linkage for what the unit
//! does not export, `nsw` on signed arithmetic, `inbounds` on pointer
//! arithmetic. The last two hold for objects of at most PTRDIFF_MAX bytes,
//! as clang assumes of every object.

use std::collections::{HashMap, HashSet};

use llrm_core::support::hash::IndexMap;
use llrm_mir::build::Builder;
use llrm_mir::{
    Attribute, BinaryOp, BlockId, CastOp, Constant, ConstantExpr, ConstantId, ConstantKind, FloatKind, FloatPredicate, Flags, GlobalId, GlobalVariable, IntPredicate,
    Linkage, MetadataId, MetadataNode, MetadataOperand, Module, Operand, Type, TypeId,
};
use num_bigint::BigInt;
use num_traits::ToPrimitive;

use crate::hir::{self, Unsupported};
use crate::raise_hir::{EMITTED, classes, far_pointers, is_float, library_routine, pointers, signed, widths};

type R<T> = Result<T, Unsupported>;

fn refuse<T>(what: impl Into<String>) -> R<T> {
    Err(Unsupported(what.into()))
}

/// A unit's MIR, and where its data goes: each data segment in the
/// stream's order, with the globals it holds, by name.
pub struct Emitted {
    pub module: Module,
    pub segments: Vec<(String, Vec<String>)>,
}

pub fn emitted(unit: &hir::Unit) -> R<Emitted> {
    let mut module = Module { datalayout: Some(llrm_core::hir::mir::DATALAYOUT.to_owned()), ..Module::default() };
    let tags = Tags::new(&mut module);
    let objects = objects(unit)?;
    let mut globals = Globals::default();
    for object in &objects {
        globals.define_data(unit, &mut module, object)?;
    }
    for symbol in unit.symbols.values().filter(|one| !one.proc() && one.imported()) {
        let ty = byte_array(&mut module, 0);
        let variable = GlobalVariable { ty, constant: false, initializer: None, align: None };
        let global = module.add_variable(&symbol.object_name(), variable, Linkage::External).map_err(Unsupported)?;
        globals.add(&mut module, Key::Symbol(symbol.id), global, if unit.grouped(symbol) { 0 } else { FAR });
    }
    for proc in &unit.procs {
        globals.define_function(unit, &mut module, proc)?;
    }
    globals.declare_callees(unit, &mut module)?;
    for object in &objects {
        let initializer = globals.initializer(&mut module, object)?;
        let global = module.named(&object.name).expect("declared above");
        let llrm_mir::GlobalKind::Variable(variable) = &mut module.globals[global.0 as usize].kind else { unreachable!("a variable") };
        variable.initializer = Some(initializer);
    }
    for proc in &unit.procs {
        let name = unit.symbols[&proc.symbol].object_name();
        let function = module.named(&name).expect("defined above");
        Body::emit(unit, proc, &globals, &tags, module.builder(function))?;
    }
    let mut segments: Vec<(String, Vec<String>)> = Vec::new();
    for object in &objects {
        match segments.iter_mut().find(|(name, _)| *name == object.segment) {
            Some((_, names)) => names.push(object.name.clone()),
            None => segments.push((object.segment.clone(), vec![object.name.clone()])),
        }
    }
    Ok(Emitted { module, segments })
}

/// A far pointer's address space, and far code's.
const FAR: u32 = 1;

fn byte_array(module: &mut Module, count: u64) -> TypeId {
    let byte = module.context.types.int(8);
    module.context.types.intern(Type::Array { element: byte, count })
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

/// Each symbol's and literal's global, and the address space it is in.
#[derive(Default)]
struct Globals {
    by_key: HashMap<Key, (GlobalId, ConstantId, u32)>,
    /// The library routines operators become, and `llvm.fabs`, by name.
    routines: HashMap<String, ConstantId>,
    /// Each defined procedure no return gives a value: a void function,
    /// which the stream types as an int.
    valueless: HashSet<i64>,
}

impl Globals {
    fn add(&mut self, module: &mut Module, key: Key, global: GlobalId, space: u32) {
        module.globals[global.0 as usize].address_space = space;
        let reference = module.reference(global);
        self.by_key.insert(key, (global, reference, space));
    }

    fn space(&self, unit: &hir::Unit, key: Key) -> u32 {
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

    fn define_data(&mut self, unit: &hir::Unit, module: &mut Module, object: &Object) -> R<()> {
        let mut fields = Vec::new();
        let mut at = 0;
        for relocation in &object.relocations {
            if relocation.at > at {
                fields.push(byte_array(module, (relocation.at - at) as u64));
            }
            fields.push(self.relocation_type(unit, module, relocation));
            at = relocation.at + if relocation.far { 4 } else { 2 };
        }
        if fields.is_empty() || object.bytes.len() > at {
            fields.push(byte_array(module, (object.bytes.len() - at) as u64));
        }
        let ty = if fields.len() == 1 { fields[0] } else { module.context.types.intern(Type::Struct { fields, packed: true }) };
        let (linkage, constant) = match object.key {
            Key::Symbol(symbol) => {
                let symbol = &unit.symbols[&symbol];
                (if symbol.exported() { Linkage::External } else { Linkage::Internal }, symbol.constant())
            }
            Key::Literal(_) => (Linkage::Private, true),
        };
        let variable = GlobalVariable { ty, constant, initializer: None, align: object.align };
        let global = module.add_variable(&object.name, variable, linkage).map_err(Unsupported)?;
        let space = self.space(unit, object.key);
        self.add(module, object.key, global, space);
        Ok(())
    }

    /// A relocated field's type: the pointer it holds, or a near one's
    /// offset word into far data.
    fn relocation_type(&self, unit: &hir::Unit, module: &mut Module, relocation: &Relocation) -> TypeId {
        let types = &mut module.context.types;
        match (relocation.far, self.space(unit, relocation.target)) {
            (true, _) => types.ptr(FAR),
            (false, FAR) => types.int(16),
            (false, _) => types.ptr(0),
        }
    }

    fn initializer(&self, module: &mut Module, object: &Object) -> R<ConstantId> {
        let context = &mut module.context;
        let bytes = |context: &mut llrm_mir::Context, slice: &[u8]| {
            let byte = context.types.int(8);
            let ty = context.types.intern(Type::Array { element: byte, count: slice.len() as u64 });
            let kind = if slice.iter().all(|&one| one == 0) { ConstantKind::Zero } else { ConstantKind::Bytes(slice.to_vec()) };
            context.constant(Constant { ty, kind })
        };
        let mut members = Vec::new();
        let mut at = 0;
        for relocation in &object.relocations {
            if relocation.at > at {
                members.push(bytes(context, &object.bytes[at..relocation.at]));
            }
            let Some(&(_, target, space)) = self.by_key.get(&relocation.target) else {
                return refuse(format!("{}: an address of {:?}, which the unit neither defines nor imports", object.name, relocation.target));
            };
            let mut address = target;
            if relocation.offset != 0 {
                let (byte, i16) = (context.types.int(8), context.types.int(16));
                let ty = context.types.ptr(space);
                let index = context.int(i16, i128::from(relocation.offset));
                let kind = ConstantKind::Expr(ConstantExpr::GetElementPtr { source: byte, inbounds: false, operands: vec![target, index] });
                address = context.constant(Constant { ty, kind });
            }
            let cast = |context: &mut llrm_mir::Context, op, ty| context.constant(Constant { ty, kind: ConstantKind::Expr(ConstantExpr::Cast { op, value: address }) });
            members.push(match (relocation.far, space) {
                (true, FAR) | (false, 0) => address,
                (true, _) => {
                    let ty = context.types.ptr(FAR);
                    cast(context, CastOp::AddrSpaceCast, ty)
                }
                (false, _) => {
                    let ty = context.types.int(16);
                    cast(context, CastOp::PtrToInt, ty)
                }
            });
            at = relocation.at + if relocation.far { 4 } else { 2 };
        }
        if members.is_empty() || object.bytes.len() > at {
            members.push(bytes(context, &object.bytes[at..]));
        }
        if members.len() == 1 {
            return Ok(members[0]);
        }
        let fields = members.iter().map(|&one| context.get(one).ty).collect();
        let ty = context.types.intern(Type::Struct { fields, packed: true });
        Ok(context.constant(Constant { ty, kind: ConstantKind::Aggregate(members) }))
    }

    fn define_function(&mut self, unit: &hir::Unit, module: &mut Module, proc: &hir::Proc) -> R<()> {
        let symbol = &unit.symbols[&proc.symbol];
        let convention = convention(symbol)?;
        let types = Types { unit };
        let valueless = proc.body.iter().filter(|one| one.call == "CGReturn").all(|one| one.args[0] == "n0");
        let returns = if valueless { module.context.types.void() } else { types.of(&mut module.context.types, &proc.type_)? };
        if valueless {
            self.valueless.insert(symbol.id);
        }
        let mut parameters = Vec::new();
        let mut first = Vec::new();
        for (_, type_) in &proc.parms {
            first.push(parameters.len());
            parameters.extend(types.passed(&mut module.context.types, type_, convention)?);
        }
        let ty = module.context.types.intern(Type::Function { returns, parameters, variadic: false });
        let linkage = if symbol.exported() { Linkage::External } else { Linkage::Internal };
        let global = module.add_function(&symbol.object_name(), ty, linkage).map_err(Unsupported)?;
        let space = self.space(unit, Key::Symbol(symbol.id));
        self.add(module, Key::Symbol(symbol.id), global, space);
        let llrm_mir::GlobalKind::Function(function) = &mut module.globals[global.0 as usize].kind else { unreachable!("a function") };
        function.calling_convention = convention;
        // C99 6.7.3.1: what a restrict parameter reaches, nothing else in its block does.
        for (at, (parameter, _)) in first.into_iter().zip(&proc.parms) {
            if restricted(unit, *parameter) {
                function.parameter_attrs[at].push(Attribute::Flag("noalias".to_owned()));
            }
        }
        Ok(())
    }

    /// Every procedure the unit names but does not define, typed by its
    /// first call; and the routines operators become.
    fn declare_callees(&mut self, unit: &hir::Unit, module: &mut Module) -> R<()> {
        let types = Types { unit };
        for call in unit.calls.values() {
            let Some(node) = unit.nodes.get(&hir::handle(&call.target)) else { continue };
            let [symbol, ..] = &node.args[..] else { continue };
            if node.call != "CGFEName" || self.by_key.contains_key(&Key::Symbol(hir::handle(symbol))) {
                continue;
            }
            let symbol = &unit.symbols[&hir::handle(symbol)];
            if !symbol.proc() || symbol.code.is_some() || EMITTED.contains(&symbol.name.as_str()) {
                continue;
            }
            let returns = types.of(&mut module.context.types, &call.type_)?;
            let convention = convention(symbol).unwrap_or(0);
            let mut parameters = Vec::new();
            for (_, type_) in call.parms.iter().rev() {
                parameters.extend(types.passed(&mut module.context.types, type_, convention)?);
            }
            let ty = module.context.types.intern(Type::Function { returns, parameters, variadic: false });
            self.declare(unit, module, symbol, ty)?;
        }
        for symbol in unit.symbols.values().filter(|one| one.proc() && one.code.is_none() && !EMITTED.contains(&one.name.as_str())) {
            if !self.by_key.contains_key(&Key::Symbol(symbol.id)) {
                let int = module.context.types.int(16);
                let ty = module.context.types.intern(Type::Function { returns: int, parameters: Vec::new(), variadic: true });
                self.declare(unit, module, symbol, ty)?;
            }
        }
        let double = module.context.types.intern(Type::Float(FloatKind::Double));
        for node in unit.nodes.values() {
            let (cg_op, arity) = match (node.call.as_str(), &node.args[..]) {
                ("CGUnary", [cg_op, _, _]) => (cg_op, 1),
                ("CGBinary", [cg_op, _, _, _]) => (cg_op, 2),
                _ => continue,
            };
            if let Some(routine) = library_routine(cg_op) {
                // Borland's library: far, cdecl, doubles in and out.
                let name = format!("_{routine}");
                if self.routines.contains_key(&name) {
                    continue;
                }
                let ty = module.context.types.intern(Type::Function { returns: double, parameters: vec![double; arity], variadic: false });
                let global = match module.named(&name) {
                    Some(global) => global,
                    None => {
                        let global = module.add_function(&name, ty, Linkage::External).map_err(Unsupported)?;
                        module.globals[global.0 as usize].address_space = FAR;
                        global
                    }
                };
                self.routines.insert(name, module.reference(global));
            }
            if cg_op == "O_FABS" && is_float(&node.args[2]) {
                let (ty, suffix) = if node.args[2] == "TY_SINGLE" { (FloatKind::Float, "f32") } else { (FloatKind::Double, "f64") };
                let name = format!("llvm.fabs.{suffix}");
                if !self.routines.contains_key(&name) {
                    let float = module.context.types.intern(Type::Float(ty));
                    let fty = module.context.types.intern(Type::Function { returns: float, parameters: vec![float], variadic: false });
                    let global = module.add_function(&name, fty, Linkage::External).map_err(Unsupported)?;
                    self.routines.insert(name, module.reference(global));
                }
            }
        }
        Ok(())
    }

    fn declare(&mut self, unit: &hir::Unit, module: &mut Module, symbol: &hir::Symbol, ty: TypeId) -> R<()> {
        let global = module.add_function(&symbol.object_name(), ty, Linkage::External).map_err(Unsupported)?;
        let space = self.space(unit, Key::Symbol(symbol.id));
        self.add(module, Key::Symbol(symbol.id), global, space);
        if let Ok(convention) = convention(symbol) {
            let llrm_mir::GlobalKind::Function(function) = &mut module.globals[global.0 as usize].kind else { unreachable!("a function") };
            function.calling_convention = convention;
        }
        Ok(())
    }
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

/// The MIR calling convention of a stack convention: cdecl's, or
/// pascal's, which pushes in order and pops its own.
fn convention(symbol: &hir::Symbol) -> R<u32> {
    let stack = symbol.call_class & (hir::CALLER_POPS | hir::REVERSE_PARMS);
    match (symbol.register_parms, stack) {
        (false, hir::CALLER_POPS) => Ok(0),
        (false, hir::REVERSE_PARMS) => Ok(llrm_mir::opcode::BASIC),
        _ => refuse(format!("{} has a register calling convention", symbol.object_name())),
    }
}

/// C's types as MIR's.
struct Types<'u> {
    unit: &'u hir::Unit,
}

impl Types<'_> {
    fn canonical(&self, type_: &str) -> String {
        self.unit.canonical_type(type_)
    }

    /// An aggregate's size; none for a scalar.
    fn aggregate(&self, type_: &str) -> Option<i64> {
        self.unit.types.get(&self.canonical(type_)).copied()
    }

    /// The arguments a parameter of `type_` is passed as: an aggregate as
    /// its words, the first at the lowest address, as cdecl pushes them.
    fn passed(&self, types: &mut llrm_mir::Types, type_: &str, convention: u32) -> R<Vec<TypeId>> {
        match self.aggregate(type_) {
            Some(_) if convention != 0 => refuse(format!("an aggregate argument by calling convention {convention}")),
            Some(size) => Ok(vec![types.int(16); ((size + 1) / 2) as usize]),
            None => Ok(vec![self.of(types, type_)?]),
        }
    }

    fn of(&self, types: &mut llrm_mir::Types, type_: &str) -> R<TypeId> {
        let type_ = self.canonical(type_);
        let big = |flag| if self.unit.target & flag != 0 { FAR } else { 0 };
        Ok(match type_.as_str() {
            "TY_POINTER" => types.ptr(big(hir::BIG_DATA)),
            "TY_CODE_PTR" => types.ptr(big(hir::BIG_CODE)),
            "TY_NEAR_POINTER" | "TY_NEAR_CODE_PTR" => types.ptr(0),
            "TY_LONG_POINTER" | "TY_HUGE_POINTER" | "TY_LONG_CODE_PTR" => types.ptr(FAR),
            "TY_SINGLE" => types.intern(Type::Float(FloatKind::Float)),
            "TY_DOUBLE" => types.intern(Type::Float(FloatKind::Double)),
            other => match (widths(other), self.aggregate(other)) {
                (Some(width), _) => types.int(width * 8),
                (None, Some(size)) => return refuse(format!("a {size}-byte aggregate as a value")),
                (None, None) => return refuse(format!("no MIR type for {other}")),
            },
        })
    }
}

/// C's aliasing classes as `!tbaa` type nodes under clang's root, each
/// under the character type, which may alias anything.
struct Tags {
    char: MetadataId,
    classes: HashMap<&'static str, MetadataId>,
}

impl Tags {
    fn new(module: &mut Module) -> Self {
        let i64 = module.context.types.int(64);
        let zero = MetadataOperand::Constant(module.context.int(i64, 0));
        let mut node = |operands: Vec<MetadataOperand>| {
            module.metadata.push(MetadataNode { distinct: false, operands });
            MetadataId(module.metadata.len() as u32 - 1)
        };
        let root = node(vec![MetadataOperand::String("Simple C/C++ TBAA".to_owned())]);
        let char_type = node(vec![MetadataOperand::String("omnipotent char".to_owned()), MetadataOperand::Node(root), zero.clone()]);
        let mut tag = |ty: MetadataId| node(vec![MetadataOperand::Node(ty), MetadataOperand::Node(ty), zero.clone()]);
        let char = tag(char_type);
        let mut classes = HashMap::new();
        for class in ["int2", "int4", "int8", "float4", "float8", "pointer2", "pointer4"] {
            let ty = node(vec![MetadataOperand::String(class.to_owned()), MetadataOperand::Node(char_type), zero.clone()]);
            classes.insert(class, node(vec![MetadataOperand::Node(ty), MetadataOperand::Node(ty), zero.clone()]));
        }
        Tags { char, classes }
    }
}

// ---- bodies ----

/// What a tree node evaluates to.
#[derive(Clone, Copy, Debug)]
enum Got {
    /// A scalar, or an lvalue's address.
    Value(Operand),
    /// An lvalue's address, accessed volatile.
    Volatile(Operand),
    /// `size` bytes of aggregate at an address.
    Aggregate(Operand, i64),
    /// A function designator, by its symbol.
    Function(i64),
    /// A call's result, before O_POINTS reads it; none from a void call.
    Returned(Option<Operand>),
}

struct Body<'a> {
    unit: &'a hir::Unit,
    proc: &'a hir::Proc,
    types: Types<'a>,
    globals: &'a Globals,
    tags: &'a Tags,
    b: Builder<'a>,
    slots: HashMap<String, Operand>,
    labels: HashMap<String, BlockId>,
    current: BlockId,
    done: HashMap<i64, Got>,
    selects: IndexMap<String, (Vec<(i64, String)>, Option<String>)>,
    returns: TypeId,
}

impl<'a> Body<'a> {
    fn emit(unit: &'a hir::Unit, proc: &'a hir::Proc, globals: &'a Globals, tags: &'a Tags, mut b: Builder<'a>) -> R<()> {
        let entry = b.block("entry");
        b.position(entry);
        let returns = b.context.types.get(b.function.ty).clone();
        let Type::Function { returns, .. } = returns else { unreachable!("a function type") };
        let mut body = Body {
            unit,
            proc,
            types: Types { unit },
            globals,
            tags,
            b,
            slots: HashMap::new(),
            labels: HashMap::new(),
            current: entry,
            done: HashMap::new(),
            selects: IndexMap::default(),
            returns,
        };
        body.frame()?;
        for one in &proc.body {
            body.statement(one)?;
        }
        if body.b.function.terminator(body.current).is_none() {
            if body.current == entry || !body.b.function.predecessors(body.current).is_empty() {
                return body.refuse("control reaches the end with no return");
            }
            body.b.unreachable();
        }
        if body.labels.values().any(|&one| body.b.function.terminator(one).is_none()) {
            return body.refuse("a label no statement places");
        }
        Ok(())
    }

    fn name(&self) -> String {
        self.unit.symbols[&self.proc.symbol].object_name()
    }

    fn refuse<T>(&self, what: impl std::fmt::Display) -> R<T> {
        refuse(format!("{}: {what}", self.name()))
    }

    fn ty(&mut self, type_: &str) -> R<TypeId> {
        let result = self.types.of(&mut self.b.context.types, type_);
        result.map_err(|error| Unsupported(format!("{}: {}", self.name(), error.0)))
    }

    fn int(&mut self, bits: u32) -> TypeId {
        self.b.context.types.int(bits)
    }

    fn type_of(&self, operand: Operand) -> TypeId {
        self.b.type_of(operand)
    }

    fn space(&self, operand: Operand) -> Option<u32> {
        match self.b.context.types.get(self.type_of(operand)) {
            Type::Pointer(space) => Some(*space),
            _ => None,
        }
    }

    fn bits(&self, operand: Operand) -> Option<u32> {
        self.b.context.types.int_bits(self.type_of(operand))
    }

    fn is_float_value(&self, operand: Operand) -> bool {
        matches!(self.b.context.types.get(self.type_of(operand)), Type::Float(_))
    }

    /// Each parameter and auto a slot; each parameter stored into its own.
    fn frame(&mut self) -> R<()> {
        let mut at = 0;
        for (symbol, type_) in &self.proc.parms {
            let slot = match self.types.aggregate(type_) {
                Some(size) => {
                    let words = (size + 1) / 2;
                    let ty = self.bytes_type(words * 2);
                    let slot = self.b.alloca(ty, "");
                    for word in 0..words {
                        let value = self.b.parameter(at);
                        let pointer = self.displaced(slot, word * 2);
                        self.b.store(value, pointer, false);
                        at += 1;
                    }
                    slot
                }
                None => {
                    let ty = self.ty(type_)?;
                    let slot = self.b.alloca(ty, "");
                    let value = self.b.parameter(at);
                    self.b.store(value, slot, false);
                    at += 1;
                    slot
                }
            };
            self.slots.insert(format!("y{symbol}"), slot);
        }
        for (key, type_) in &self.proc.autos {
            let ty = match self.types.aggregate(type_) {
                Some(size) => self.bytes_type(size),
                None => self.ty(type_)?,
            };
            let slot = self.b.alloca(ty, "");
            self.slots.insert(key.clone(), slot);
        }
        Ok(())
    }

    /// `pointer` moved `by` constant bytes, inside its object.
    fn displaced(&mut self, pointer: Operand, by: i64) -> Operand {
        if by == 0 {
            return pointer;
        }
        let (byte, at) = (self.int(8), self.b.int(16, i128::from(by)));
        self.b.gep(byte, pointer, &[at], Flags::INBOUNDS, "")
    }

    fn bytes_type(&mut self, count: i64) -> TypeId {
        let byte = self.int(8);
        self.b.context.types.intern(Type::Array { element: byte, count: count as u64 })
    }

    // ---- blocks ----

    fn label(&mut self, name: &str) -> BlockId {
        if let Some(&block) = self.labels.get(name) {
            return block;
        }
        let block = self.b.block(name);
        self.labels.insert(name.to_owned(), block);
        block
    }

    /// Code from here on goes into `block`, which the code before falls into.
    fn place(&mut self, block: BlockId) {
        if self.b.function.terminator(self.current).is_none() {
            self.b.br(block);
        }
        self.b.position(block);
        self.current = block;
    }

    /// A block for code after a terminator, which nothing reaches unless a label follows.
    fn open(&mut self) {
        if self.b.function.terminator(self.current).is_some() {
            let block = self.b.block("");
            self.b.position(block);
            self.current = block;
        }
    }

    // ---- statements ----

    fn statement(&mut self, one: &hir::Statement) -> R<()> {
        let args: Vec<&str> = one.args.iter().map(String::as_str).collect();
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
                self.b.br(block);
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
                let wide = self.int(bits);
                let value = self.resized(value, signed(&self.types.canonical(&from)), wide);
                let default = self.label(&other);
                let mut arms = Vec::new();
                for (case, label) in cases {
                    let block = self.label(&label);
                    arms.push((self.b.int(bits, i128::from(case)), block));
                }
                self.b.switch(value, default, &arms);
            }
            _ => return refuse(format!("{} line {}: {} {}", self.name(), one.line, one.call, one.args.join(" "))),
        }
        Ok(())
    }

    fn ret(&mut self, node: &str, type_: &str) -> R<()> {
        if self.b.context.types.get(self.returns) == &Type::Void {
            if node != "n0" {
                self.eval(node)?;
            }
            self.b.ret(None);
            return Ok(());
        }
        if node == "n0" {
            // A value-less return from a function that has one: nothing the caller may read.
            let poison = self.b.context.constant(Constant { ty: self.returns, kind: ConstantKind::Poison });
            self.b.ret(Some(Operand::Constant(poison)));
            return Ok(());
        }
        let value = self.value(node, type_)?;
        let value = self.converted(value, type_, &self.proc.type_.clone())?;
        self.b.ret(Some(value));
        Ok(())
    }

    /// Go to `target` when `node` is `when`; fall through otherwise.
    fn branch(&mut self, node: &str, target: BlockId, when: bool) -> R<()> {
        let tree = &self.unit.nodes[&hir::handle(node)];
        let args: Vec<&str> = tree.args.iter().map(String::as_str).collect();
        match (tree.call.as_str(), &args[..]) {
            ("CGFlow", ["O_FLOW_NOT", inner, _]) => return self.branch(inner, target, !when),
            ("CGFlow", [flow @ ("O_FLOW_AND" | "O_FLOW_OR"), left, right]) => {
                if (*flow == "O_FLOW_OR") == when {
                    self.branch(left, target, when)?;
                    return self.branch(right, target, when);
                }
                let skip = self.b.block("");
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
        let fall = self.b.block("");
        let (taken, otherwise) = if when { (target, fall) } else { (fall, target) };
        self.b.cond_br(condition, taken, otherwise);
        self.b.position(fall);
        self.current = fall;
        Ok(())
    }

    /// `value != 0`, as C tests a scalar.
    fn nonzero(&mut self, value: Operand) -> Operand {
        let ty = self.type_of(value);
        if self.is_float_value(value) {
            let zero = self.b.context.constant(Constant { ty, kind: ConstantKind::Float(0) });
            return self.b.fcmp(FloatPredicate::Une, value, Operand::Constant(zero), "");
        }
        let value = match self.space(value) {
            Some(FAR) => {
                let dword = self.int(32);
                self.b.cast(CastOp::PtrToInt, value, dword, "")
            }
            _ => value,
        };
        let ty = self.type_of(value);
        let zero = match self.b.context.types.get(ty) {
            Type::Pointer(_) => self.b.context.constant(Constant { ty, kind: ConstantKind::Null }),
            _ => self.b.context.int(ty, 0),
        };
        self.b.icmp(IntPredicate::Ne, value, Operand::Constant(zero), "")
    }

    fn compare(&mut self, args: &[String]) -> R<Operand> {
        let [cg_op, left, right, type_] = args else { return self.refuse("a compare of no three operands") };
        let a = self.value(left, type_)?;
        let b = self.value(right, type_)?;
        if is_float(&self.types.canonical(type_)) {
            let predicate = match cg_op.as_str() {
                "O_EQ" => FloatPredicate::Oeq,
                "O_NE" => FloatPredicate::Une,
                "O_LT" => FloatPredicate::Olt,
                "O_LE" => FloatPredicate::Ole,
                "O_GT" => FloatPredicate::Ogt,
                "O_GE" => FloatPredicate::Oge,
                other => return self.refuse(format!("float compare {other}")),
            };
            return Ok(self.b.fcmp(predicate, a, b, ""));
        }
        // A far pointer compares as its dword, as the old raise does.
        let (a, b) = match self.space(a) {
            Some(FAR) => {
                let dword = self.int(32);
                (self.b.cast(CastOp::PtrToInt, a, dword, ""), self.b.cast(CastOp::PtrToInt, b, dword, ""))
            }
            _ => (a, b),
        };
        let is_signed = signed(&self.types.canonical(type_));
        let predicate = match (cg_op.as_str(), is_signed) {
            ("O_EQ", _) => IntPredicate::Eq,
            ("O_NE", _) => IntPredicate::Ne,
            ("O_LT", true) => IntPredicate::Slt,
            ("O_LE", true) => IntPredicate::Sle,
            ("O_GT", true) => IntPredicate::Sgt,
            ("O_GE", true) => IntPredicate::Sge,
            ("O_LT", false) => IntPredicate::Ult,
            ("O_LE", false) => IntPredicate::Ule,
            ("O_GT", false) => IntPredicate::Ugt,
            ("O_GE", false) => IntPredicate::Uge,
            (other, _) => return self.refuse(format!("compare {other}")),
        };
        Ok(self.b.icmp(predicate, a, b, ""))
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
    fn value(&mut self, node: &str, type_: &str) -> R<Operand> {
        let got = self.eval(node)?;
        let value = self.scalar(got)?;
        let from = self.type_of_node(node);
        self.converted(value, &from, type_)
    }

    /// A node's result used as a value: an address is the pointer, a
    /// function decays to its address, a call's result is itself.
    fn scalar(&mut self, got: Got) -> R<Operand> {
        match got {
            Got::Value(value) | Got::Volatile(value) | Got::Aggregate(value, _) | Got::Returned(Some(value)) => Ok(value),
            Got::Function(symbol) => Ok(Operand::Constant(self.globals.by_key[&Key::Symbol(symbol)].1)),
            Got::Returned(None) => self.refuse("a void call's value"),
        }
    }

    /// An lvalue's address, and whether it is accessed volatile.
    fn address(&mut self, got: Got) -> R<(Operand, bool)> {
        let (value, volatile) = match got {
            Got::Volatile(value) => (value, true),
            got => (self.scalar(got)?, false),
        };
        match (self.space(value), self.bits(value)) {
            (Some(_), _) => Ok((value, volatile)),
            (None, Some(32)) => {
                let far = self.b.context.types.ptr(FAR);
                Ok((self.b.cast(CastOp::IntToPtr, value, far, ""), volatile))
            }
            (None, Some(_)) => {
                let word = self.int(16);
                let value = self.resized(value, false, word);
                let near = self.b.context.types.ptr(0);
                Ok((self.b.cast(CastOp::IntToPtr, value, near, ""), volatile))
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
                let ty = self.ty(type_)?;
                let value: BigInt = value.trim().parse().map_err(|_| Unsupported(format!("not an integer: {value}")))?;
                match self.b.context.types.int_bits(ty) {
                    Some(bits) => {
                        let wrapped: BigInt = value & ((BigInt::from(1) << bits) - BigInt::from(1));
                        Got::Value(self.b.int(bits, wrapped.to_i128().expect("a wrapped integer fits")))
                    }
                    None => {
                        let word = self.int(16);
                        let value = self.b.context.int(word, value.to_i128().unwrap_or(0));
                        Got::Value(self.converted(Operand::Constant(value), "TY_UINT_2", type_)?)
                    }
                }
            }
            ("CGFloat", [text, type_]) if is_float(type_) => {
                let value: f64 = text.trim().trim_matches('"').parse().map_err(|_| Unsupported(format!("not a float: {text}")))?;
                let ty = self.ty(type_)?;
                let bits = if *type_ == "TY_SINGLE" { u64::from((value as f32).to_bits()) } else { value.to_bits() };
                Got::Value(Operand::Constant(self.b.context.constant(Constant { ty, kind: ConstantKind::Float(bits) })))
            }
            ("CGFEName", [symbol, _]) => self.named(symbol)?,
            ("CGTempName", [temp, _]) => match self.slots.get(*temp) {
                Some(&slot) => Got::Value(slot),
                None => return self.refuse(format!("no temporary {temp}")),
            },
            ("CGBackName", [back, _]) => match back_key(self.unit, hir::handle(back)) {
                Key::Symbol(symbol) => self.named(&format!("y{symbol}"))?,
                key => match self.globals.by_key.get(&key) {
                    Some(&(_, reference, _)) => Got::Value(Operand::Constant(reference)),
                    None => return self.refuse(format!("literal {back} is in no data segment")),
                },
            },
            ("CGUnary", ["O_POINTS", inner, type_]) => {
                let got = self.eval(inner)?;
                self.points(got, type_)?
            }
            ("CGUnary", ["O_CONVERT", inner, type_]) => {
                let value = self.value(inner, type_)?;
                Got::Value(value)
            }
            ("CGUnary", [cg_op, inner, type_]) if library_routine(cg_op).is_some() => {
                let argument = self.value(inner, "TY_DOUBLE")?;
                let result = self.routine(cg_op, &[argument])?;
                Got::Value(self.converted(result, "TY_DOUBLE", type_)?)
            }
            ("CGUnary", [cg_op, inner, type_]) => {
                let value = self.value(inner, type_)?;
                Got::Value(self.unary(cg_op, value, type_)?)
            }
            ("CGBinary", ["O_COMMA", left, right, _]) => {
                self.eval(left)?;
                self.eval(right)?
            }
            ("CGBinary", [cg_op, left, right, type_]) if library_routine(cg_op).is_some() => {
                // The runtime takes them last first, as every call's parms are listed.
                let second = self.value(right, "TY_DOUBLE")?;
                let first = self.value(left, "TY_DOUBLE")?;
                let result = self.routine(cg_op, &[first, second])?;
                Got::Value(self.converted(result, "TY_DOUBLE", type_)?)
            }
            ("CGBinary", [cg_op, left, right, type_]) => Got::Value(self.binary(cg_op, left, right, type_)?),
            ("CGAssign", [target, source, type_]) => {
                let value = self.value(source, type_)?;
                let got = self.eval(target)?;
                let (pointer, volatile) = self.address(got)?;
                self.store(value, pointer, volatile, type_);
                Got::Value(value)
            }
            ("CGLVAssign", [target, source, _]) => {
                let Got::Aggregate(from, size) = self.eval(source)? else { return self.refuse("an aggregate assignment from a scalar") };
                let got = self.eval(target)?;
                let (into, _) = self.address(got)?;
                self.copy(into, from, size);
                Got::Aggregate(into, size)
            }
            ("CGPostGets" | "CGPreGets", [cg_op, target, source, type_]) => {
                let got = self.eval(target)?;
                let (pointer, volatile) = self.address(got)?;
                let ty = self.ty(type_)?;
                let old = self.load(ty, pointer, volatile, type_);
                let new = if self.space(old).is_some() {
                    let from = self.type_of_node(source);
                    let got = self.eval(source)?;
                    let by = self.scalar(got)?;
                    self.moved(old, by, &from, *cg_op == "O_MINUS")?
                } else {
                    let by = self.value(source, type_)?;
                    self.arithmetic(cg_op, old, by, type_)?
                };
                self.store(new, pointer, volatile, type_);
                Got::Value(if call == "CGPostGets" { old } else { new })
            }
            ("CGCall", [call]) => {
                let call = &self.unit.calls[&hir::handle(call)];
                self.call(call)?
            }
            ("CGChoose", [test, yes, no, type_]) => {
                let ty = self.ty(type_)?;
                let otherwise = self.b.block("");
                let join = self.b.block("");
                self.branch(test, otherwise, false)?;
                let a = self.value(yes, type_)?;
                let from_yes = self.current;
                self.b.br(join);
                self.b.position(otherwise);
                self.current = otherwise;
                let b = self.value(no, type_)?;
                let from_no = self.current;
                self.b.br(join);
                self.b.position(join);
                self.current = join;
                Got::Value(self.b.phi(ty, &[(a, from_yes), (b, from_no)], ""))
            }
            ("CGCompare", _) => {
                let condition = self.compare(&tree.args)?;
                let word = self.int(16);
                Got::Value(self.b.cast(CastOp::ZExt, condition, word, ""))
            }
            ("CGFlow", _) => {
                let no = self.b.block("");
                let join = self.b.block("");
                self.branch(node, no, false)?;
                let from_yes = self.current;
                self.b.br(join);
                self.b.position(no);
                self.b.br(join);
                self.b.position(join);
                self.current = join;
                let word = self.int(16);
                let (one, zero) = (self.b.int(16, 1), self.b.int(16, 0));
                Got::Value(self.b.phi(word, &[(one, from_yes), (zero, no)], ""))
            }
            ("CGEval", [inner]) | ("CGAttr", [inner, _]) => self.eval(inner)?,
            ("CGVolatile", [inner]) => {
                let got = self.eval(inner)?;
                Got::Volatile(self.address(got)?.0)
            }
            _ => return self.refuse(format!("{} {}", tree.call, tree.args.join(" "))),
        })
    }

    /// A symbol's lvalue: its slot, its global's address, or the function.
    fn named(&mut self, token: &str) -> R<Got> {
        if let Some(&slot) = self.slots.get(token) {
            return Ok(Got::Value(slot));
        }
        let symbol = &self.unit.symbols[&hir::handle(token)];
        if symbol.proc() {
            if symbol.code.is_some() || EMITTED.contains(&symbol.name.as_str()) {
                return self.refuse(format!("inline code {} is not raised to the rich MIR", symbol.name));
            }
            return Ok(Got::Function(symbol.id));
        }
        match self.globals.by_key.get(&Key::Symbol(symbol.id)) {
            Some(&(_, reference, _)) => Ok(Got::Value(Operand::Constant(reference))),
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
        let ty = self.ty(type_)?;
        Ok(Got::Value(self.load(ty, pointer, volatile, type_)))
    }

    /// The `!tbaa` tag of an access as `type_`: its C aliasing class, or
    /// the character type's, which may alias anything.
    fn tag(&self, type_: &str) -> MetadataId {
        let type_ = self.types.canonical(type_);
        let class = match type_.as_str() {
            "TY_POINTER" => Some(if self.unit.target & hir::BIG_DATA != 0 { "pointer4" } else { "pointer2" }),
            other => classes(other),
        };
        class.map_or(self.tags.char, |one| self.tags.classes[one])
    }

    fn load(&mut self, ty: TypeId, pointer: Operand, volatile: bool, type_: &str) -> Operand {
        let value = self.b.load(ty, pointer, volatile, "");
        let tag = self.tag(type_);
        self.b.attach("tbaa", tag);
        value
    }

    fn store(&mut self, value: Operand, pointer: Operand, volatile: bool, type_: &str) {
        self.b.store(value, pointer, volatile);
        let tag = self.tag(type_);
        self.b.attach("tbaa", tag);
    }

    /// `size` bytes from `from` to `into`, widest first, as the old raise copies.
    fn copy(&mut self, into: Operand, from: Operand, size: i64) {
        let mut done = 0;
        while done < size {
            let width = [4, 2, 1].into_iter().find(|&one| size - done >= one).expect("a byte left");
            let ty = self.int(width as u32 * 8);
            let source = self.displaced(from, done);
            let target = self.displaced(into, done);
            let moved = self.b.load(ty, source, false, "");
            self.b.store(moved, target, false);
            done += width;
        }
    }

    /// `value`, of C type `from`, as `to`.
    fn converted(&mut self, value: Operand, from: &str, to: &str) -> R<Operand> {
        let ty = self.ty(to)?;
        if self.type_of(value) == ty {
            return Ok(value);
        }
        let (from, to) = (self.types.canonical(from), self.types.canonical(to));
        let source_signed = signed(&from);
        let target = self.b.context.types.get(ty).clone();
        let source = self.b.context.types.get(self.type_of(value)).clone();
        Ok(match (source, target) {
            (Type::Int(_), Type::Int(_)) => self.resized(value, source_signed, ty),
            (Type::Int(bits), Type::Float(_)) => {
                if source_signed {
                    self.b.cast(CastOp::SIToFP, value, ty, "")
                } else if bits < 64 {
                    // No unsigned load: its zero extension, one width up, is signed.
                    let wide = self.int(if bits < 32 { 32 } else { 64 });
                    let wide = self.resized(value, false, wide);
                    self.b.cast(CastOp::SIToFP, wide, ty, "")
                } else {
                    self.b.cast(CastOp::UIToFP, value, ty, "")
                }
            }
            (Type::Float(_), Type::Int(_)) => self.b.cast(if signed(&to) { CastOp::FPToSI } else { CastOp::FPToUI }, value, ty, ""),
            (Type::Float(a), Type::Float(b)) => self.b.cast(if a == FloatKind::Float && b == FloatKind::Double { CastOp::FPExt } else { CastOp::FPTrunc }, value, ty, ""),
            (Type::Pointer(a), Type::Pointer(b)) if a == b => value,
            (Type::Pointer(_), Type::Pointer(_)) => self.b.cast(CastOp::AddrSpaceCast, value, ty, ""),
            (Type::Int(_), Type::Pointer(space)) => {
                // A word is a near pointer, into DGROUP; a dword segment:offset.
                let (bits, near) = if space == FAR && self.bits(value).is_some_and(|one| one >= 32) { (32, false) } else { (16, true) };
                let int = self.int(bits);
                let value = self.resized(value, false, int);
                if near {
                    let ptr = self.b.context.types.ptr(0);
                    let near = self.b.cast(CastOp::IntToPtr, value, ptr, "");
                    if space == 0 { near } else { self.b.cast(CastOp::AddrSpaceCast, near, ty, "") }
                } else {
                    self.b.cast(CastOp::IntToPtr, value, ty, "")
                }
            }
            (Type::Pointer(space), Type::Int(_)) => {
                let int = self.int(if space == FAR { 32 } else { 16 });
                let whole = self.b.cast(CastOp::PtrToInt, value, int, "");
                self.resized(whole, false, ty)
            }
            (source, target) => return self.refuse(format!("a conversion from {source:?} to {target:?}")),
        })
    }

    /// An integer at another width: extended as its signedness says, or truncated.
    fn resized(&mut self, value: Operand, is_signed: bool, ty: TypeId) -> Operand {
        let (from, to) = (self.bits(value).expect("an integer"), self.b.context.types.int_bits(ty).expect("an integer type"));
        match from.cmp(&to) {
            std::cmp::Ordering::Equal => value,
            std::cmp::Ordering::Less => self.b.cast(if is_signed { CastOp::SExt } else { CastOp::ZExt }, value, ty, ""),
            std::cmp::Ordering::Greater => self.b.cast(CastOp::Trunc, value, ty, ""),
        }
    }

    fn unary(&mut self, cg_op: &str, value: Operand, type_: &str) -> R<Operand> {
        let floats = is_float(&self.types.canonical(type_));
        Ok(match (cg_op, floats) {
            ("O_UMINUS", true) => self.b.fneg(value, ""),
            ("O_FABS", true) => {
                let name = if self.types.canonical(type_) == "TY_SINGLE" { "llvm.fabs.f32" } else { "llvm.fabs.f64" };
                let ty = self.type_of(value);
                let fty = self.b.context.types.intern(Type::Function { returns: ty, parameters: vec![ty], variadic: false });
                let callee = Operand::Constant(self.globals.routines[name]);
                self.b.call(fty, callee, &[value], "").expect("fabs's value")
            }
            ("O_UMINUS", false) => {
                let bits = self.bits(value).ok_or_else(|| Unsupported(format!("{}: negation of a non-integer", self.name())))?;
                let zero = self.b.int(bits, 0);
                let flags = self.wraps(type_, bits);
                self.b.binary(BinaryOp::Sub, zero, value, flags, "")
            }
            ("O_COMPLEMENT", false) => {
                let bits = self.bits(value).ok_or_else(|| Unsupported(format!("{}: complement of a non-integer", self.name())))?;
                let ones = self.b.int(bits, -1);
                self.b.binary(BinaryOp::Xor, value, ones, Flags::default(), "")
            }
            (other, _) => return self.refuse(format!("{other} of {type_}")),
        })
    }

    /// `nsw` where C promises a signed result fits: an int or wider.
    fn wraps(&self, type_: &str, bits: u32) -> Flags {
        if signed(&self.types.canonical(type_)) && bits >= 16 { Flags::NSW } else { Flags::default() }
    }

    fn binary(&mut self, cg_op: &str, left: &str, right: &str, type_: &str) -> R<Operand> {
        let canonical = self.types.canonical(type_);
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
                    return Ok(self.b.binary(BinaryOp::Sub, a, b, Flags::default(), ""));
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
        let a = self.value(left, type_)?;
        let b = if matches!(cg_op, "O_LSHIFT" | "O_RSHIFT") {
            let got = self.eval(right)?;
            let count = self.scalar(got)?;
            let ty = self.type_of(a);
            match self.b.context.types.int_bits(ty) {
                Some(_) => self.resized(count, false, ty),
                None => return self.refuse(format!("a shift of {type_}")),
            }
        } else {
            self.value(right, type_)?
        };
        self.arithmetic(cg_op, a, b, type_)
    }

    /// `pointer` moved by `by` bytes, of C type `from`: inbounds, as C
    /// keeps pointer arithmetic inside its object.
    fn moved(&mut self, pointer: Operand, by: Operand, from: &str, subtract: bool) -> R<Operand> {
        let word = self.int(16);
        let by = match self.bits(by) {
            Some(_) => self.resized(by, signed(&self.types.canonical(from)), word),
            None => return self.refuse(format!("a pointer moved by a {from}")),
        };
        let by = if subtract {
            let zero = self.b.int(16, 0);
            self.b.binary(BinaryOp::Sub, zero, by, Flags::default(), "")
        } else {
            by
        };
        let byte = self.int(8);
        Ok(self.b.gep(byte, pointer, &[by], Flags::INBOUNDS, ""))
    }

    fn arithmetic(&mut self, cg_op: &str, a: Operand, b: Operand, type_: &str) -> R<Operand> {
        if is_float(&self.types.canonical(type_)) {
            let op = match cg_op {
                "O_PLUS" => BinaryOp::FAdd,
                "O_MINUS" => BinaryOp::FSub,
                "O_TIMES" => BinaryOp::FMul,
                "O_DIV" => BinaryOp::FDiv,
                other => return self.refuse(format!("float {other}")),
            };
            return Ok(self.b.binary(op, a, b, Flags::default(), ""));
        }
        let Some(bits) = self.bits(a) else { return self.refuse(format!("{cg_op} of {type_}")) };
        let is_signed = signed(&self.types.canonical(type_));
        let (op, flags) = match cg_op {
            "O_PLUS" => (BinaryOp::Add, self.wraps(type_, bits)),
            "O_MINUS" => (BinaryOp::Sub, self.wraps(type_, bits)),
            "O_TIMES" => (BinaryOp::Mul, self.wraps(type_, bits)),
            "O_AND" => (BinaryOp::And, Flags::default()),
            "O_OR" => (BinaryOp::Or, Flags::default()),
            "O_XOR" => (BinaryOp::Xor, Flags::default()),
            "O_LSHIFT" => (BinaryOp::Shl, Flags::default()),
            "O_RSHIFT" => (if is_signed { BinaryOp::AShr } else { BinaryOp::LShr }, Flags::default()),
            "O_DIV" | "O_MOD" => {
                let op = match (cg_op, is_signed) {
                    ("O_DIV", true) => BinaryOp::SDiv,
                    ("O_DIV", false) => BinaryOp::UDiv,
                    (_, true) => BinaryOp::SRem,
                    (_, false) => BinaryOp::URem,
                };
                if bits < 16 {
                    // A byte's quotient at a word, as C promotes one.
                    let word = self.int(16);
                    let (x, y) = (self.resized(a, is_signed, word), self.resized(b, is_signed, word));
                    let wide = self.b.binary(op, x, y, Flags::default(), "");
                    let ty = self.type_of(a);
                    return Ok(self.b.cast(CastOp::Trunc, wide, ty, ""));
                }
                (op, Flags::default())
            }
            other => return self.refuse(other.to_owned()),
        };
        Ok(self.b.binary(op, a, b, flags, ""))
    }

    /// A library routine's double result for an operator.
    fn routine(&mut self, cg_op: &str, arguments: &[Operand]) -> R<Operand> {
        let name = format!("_{}", library_routine(cg_op).expect("a routine"));
        let double = self.b.context.types.intern(Type::Float(FloatKind::Double));
        let fty = self.b.context.types.intern(Type::Function { returns: double, parameters: vec![double; arguments.len()], variadic: false });
        let callee = Operand::Constant(self.globals.routines[&name]);
        Ok(self.b.call(fty, callee, arguments, "").expect("a double"))
    }

    fn call(&mut self, call: &hir::Call) -> R<Got> {
        let target = self.eval(&call.target)?;
        let convention = match target {
            Got::Function(symbol) => convention(&self.unit.symbols[&symbol]),
            _ => convention(&self.unit.symbols[&call.symbol]),
        }
        .map_err(|error| Unsupported(format!("{}: {}", self.name(), error.0)))?;
        let callee = self.scalar(target)?;
        // Evaluated as listed, last first; passed first first.
        let mut arguments = Vec::new();
        for (node, type_) in &call.parms {
            let Some(size) = self.types.aggregate(type_) else {
                arguments.push(self.value(node, type_)?);
                continue;
            };
            if convention != 0 {
                return self.refuse(format!("an aggregate argument by calling convention {convention}"));
            }
            let Got::Aggregate(from, _) = self.eval(node)? else { return self.refuse("a scalar passed as an aggregate") };
            // Its words, last first as the arguments are listed; a last odd byte widened.
            let word = self.int(16);
            for at in (0..(size + 1) / 2).rev().map(|one| one * 2) {
                let pointer = self.displaced(from, at);
                let value = if at + 1 < size {
                    self.b.load(word, pointer, false, "")
                } else {
                    let byte = self.int(8);
                    let value = self.b.load(byte, pointer, false, "");
                    self.b.cast(CastOp::ZExt, value, word, "")
                };
                arguments.push(value);
            }
        }
        arguments.reverse();
        let returns = match target {
            Got::Function(symbol) if self.globals.valueless.contains(&symbol) => self.b.context.types.void(),
            _ => self.ty(&call.type_)?,
        };
        let parameters = arguments.iter().map(|&one| self.type_of(one)).collect();
        let fty = self.b.context.types.intern(Type::Function { returns, parameters, variadic: false });
        Ok(Got::Returned(self.b.call_as(convention, fty, callee, &arguments, "")))
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use llrm_mir::{Attribute, Linkage, Module};

    use super::emitted;
    use crate::{hir, stream};

    fn raised(fixture: &str) -> Module {
        let text = std::fs::read_to_string(Path::new(env!("LLRM_ROOT")).join("tests/fixtures/c").join(fixture)).unwrap();
        emitted(&hir::unit(&stream::parse(&text)).unwrap()).unwrap().module
    }

    /// `@name`'s definition as printed.
    fn defined(module: &Module, name: &str) -> String {
        let text = llrm_mir::print::module(module);
        let start = text[..text.find(&format!("@{name}(")).unwrap()].rfind("define").unwrap();
        let length = text[start..].find("\n}").unwrap();
        text[start..start + length].to_owned()
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
        let text = defined(&raised("bytes.cgs"), "_bytes");
        let adds: Vec<&str> = text.lines().filter(|one| one.contains(" = add ")).collect();
        assert!(adds.iter().any(|one| one.contains("add nsw i16")) && adds.iter().any(|one| one.contains("add i16")), "{adds:#?}");
        assert!(text.lines().filter(|one| one.contains("getelementptr")).all(|one| one.contains("getelementptr inbounds")), "{text}");
        assert!(text.lines().filter(|one| one.contains("load i8")).all(|one| one.contains("!tbaa !2")), "{text}");
    }

    /// `*seed` is a short's access: C's int2 class, not the character type.
    #[test]
    fn test_accesses_carry_their_aliasing_class() {
        let module = raised("parity/loop.cgs");
        let text = defined(&module, "_parity_loop");
        let printed = llrm_mir::print::module(&module);
        let int2 = printed.lines().find(|one| one.contains("!\"int2\"")).unwrap();
        let int2 = &int2[..int2.find(' ').unwrap()];
        let tag = printed.lines().find(|one| one.contains(&format!("= !{{{int2}, {int2}, i64 0}}"))).unwrap();
        let tag = &tag[..tag.find(' ').unwrap()];
        assert!(text.lines().any(|one| one.contains("load i16") && one.ends_with(&format!("!tbaa {tag}"))), "{text}\n{printed}");
    }
}
