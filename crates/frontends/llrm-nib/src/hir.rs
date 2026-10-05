use std::fmt::Write;

use super::syntax::Abi;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Type {
    pub id: u32,
    pub name: String,
    pub kind: &'static str,
    pub width: u32,
    pub signed: Option<bool>,
    pub evaluation: &'static str,
    pub element: Option<u32>,
    pub rank: u32,
    pub bounds: Vec<(i32, i32)>,
    pub address: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Value {
    pub id: u32,
    pub type_id: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Place {
    pub id: u32,
    pub name: String,
    pub type_id: u32,
    pub mutable: bool,
    pub offset: i32,
    pub extent: u32,
    pub storage: &'static str,
    pub symbol: u32,
    pub volatile: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataObject {
    pub id: u32,
    pub name: String,
    pub bytes: Vec<u8>,
    pub readonly: bool,
    /// The callable whose far address its bytes hold, when they hold one.
    pub code: Option<u32>,
    /// A huge object's segment of its own, the first of as many as it fills.
    pub segment: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Operand {
    Value(u32),
    Constant(u32, i64),
    Place(u32),
    ArrayElement(u32, Vec<Operand>),
    ProjectedPlace {
        place: u32,
        indices: Vec<Operand>,
        offset: u32,
        type_id: u32,
        /// The member of an aggregate type this is: the type and the member's offset in it.
        member: Option<(u32, u32)>,
    },
    IndirectPlace {
        base: u32,
        offset: u32,
        type_id: u32,
        // An array element: the language promises it stays inside its array.
        inbounds: bool,
        /// The member of an aggregate type this is: the type and the member's offset in it.
        member: Option<(u32, u32)>,
    },
    DescriptorPlace {
        base: u32,
        field: &'static str,
        type_id: u32,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub id: u32,
    pub op: &'static str,
    pub results: Vec<u32>,
    pub operands: Vec<Operand>,
    pub callee: Option<String>,
    pub asm: Option<Asm>,
    /// The source line of the statement it belongs to, 0 for none.
    pub line: u32,
    /// An address the frontend knows lies in the object its operand points
    /// into: the borrow it indexes bounds it.
    pub inbounds: bool,
}

/// An `asm` instruction's code and the 16-bit registers it reads and writes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Asm {
    pub code: Vec<u8>,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    pub clobbers: Vec<String>,
    pub memory: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Terminator {
    pub kind: &'static str,
    pub operands: Vec<Operand>,
    pub targets: Vec<u32>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub id: u32,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallSite {
    pub instruction: u32,
    pub order: Vec<u32>,
    pub callee: u32,
    pub callee_cleans: bool,
    pub float_return: &'static str,
}

impl CallSite {
    /// A call of `count` arguments, pushed as `abi` orders them.
    pub fn new(instruction: u32, callee: u32, count: u32, abi: Abi) -> Self {
        Self {
            instruction,
            order: if abi.callee_cleans() { (0..count).collect() } else { (0..count).rev().collect() },
            callee,
            callee_cleans: abi.callee_cleans(),
            float_return: abi.float_return(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Function {
    pub id: u32,
    pub name: String,
    pub result_type: u32,
    pub values: Vec<Value>,
    pub places: Vec<Place>,
    pub blocks: Vec<Block>,
    pub entry: u32,
    pub parameters: Vec<u32>,
    pub calls: Vec<CallSite>,
    /// Callable from other objects by its symbol.
    pub exported: bool,
    /// How it is entered and left, when not as a native function is.
    pub abi: Option<ProcedureAbi>,
    /// What the language promises of it, as the one API states it.
    pub facts: Vec<llrm_core::hir::facts::Stated>,
    /// `-g`: each source parameter's value and name.
    pub named_parameters: Vec<(u32, String)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProcedureAbi {
    pub distance: &'static str,
    pub cleanup: &'static str,
    /// The argument bytes it removes on return.
    pub parameter_bytes: u32,
    pub float_return: &'static str,
}

impl ProcedureAbi {
    /// A function of `abi`, taking `argument_bytes`, when it differs from a
    /// native one: it removes its arguments, or it returns with `iret`.
    /// An unexported one states no distance (`any`): only its own module calls it.
    pub fn of(abi: Abi, argument_bytes: u32, exported: bool) -> Option<Self> {
        let distance = if exported { "far" } else { "any" };
        match abi {
            Abi::Cdecl16 => (!exported).then(|| Self { distance, cleanup: "caller", parameter_bytes: 0, float_return: abi.float_return() }),
            Abi::Pascal16 | Abi::Basic(_) => Some(Self { distance, cleanup: "callee", parameter_bytes: argument_bytes, float_return: abi.float_return() }),
            Abi::Interrupt16 => Some(Self { distance: "interrupt", cleanup: "callee", parameter_bytes: 0, float_return: abi.float_return() }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Callable {
    pub id: u32,
    pub name: String,
    pub result_type: Option<u32>,
    pub parameter_types: Vec<u32>,
    pub defined: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Program {
    pub module_name: String,
    pub types: Vec<Type>,
    pub functions: Vec<Function>,
    pub callables: Vec<Callable>,
    /// What the program states of its routines, apart from each function's own.
    pub facts: Vec<llrm_core::hir::facts::Stated>,
    pub data: Vec<DataObject>,
    /// `-g`: what a debugger names and how it reads it.
    pub debug: Option<llrm_core::hir::model::Debug>,
}

impl Program {
    /// The common HIR wire format is intentionally written without a serde
    /// dependency so the standalone frontend remains one small binary.
    pub fn json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"array_order\":\"row-major\",\"dialect\":\"nib\",\"float_mode\":\"inline\",\"modules\":[{");
        out.push_str("\"callables\":[");
        for (index, callable) in self.callables.iter().enumerate() {
            comma(&mut out, index);
            write!(out, "{{\"arrays\":[").unwrap();
            booleans(&mut out, callable.parameter_types.len(), false);
            write!(out, "],\"by_value\":[").unwrap();
            booleans(&mut out, callable.parameter_types.len(), true);
            write!(
                out,
                "],\"defined\":{},\"id\":{},\"name\":",
                callable.defined, callable.id
            )
            .unwrap();
            string(&mut out, &callable.name);
            out.push_str(",\"parameter_types\":[");
            numbers(&mut out, &callable.parameter_types);
            out.push_str("],\"result_type\":");
            optional_number(&mut out, callable.result_type);
            out.push_str(",\"segmented\":[");
            booleans(&mut out, callable.parameter_types.len(), false);
            out.push_str("]}");
        }
        out.push_str("],\"data\":[");
        for (index, object) in self.data.iter().enumerate() {
            comma(&mut out, index);
            write!(out, "{{\"address\":\"{}\",\"bytes\":[", if object.segment.is_some() { "huge" } else { "near" }).unwrap();
            bytes(&mut out, &object.bytes);
            write!(
                out,
                "],\"id\":{},\"linkage\":\"internal\",\"name\":",
                object.id
            )
            .unwrap();
            string(&mut out, &object.name);
            if let Some(segment) = &object.segment {
                out.push_str(",\"segment\":");
                string(&mut out, segment);
            }
            write!(out, ",\"readonly\":{},\"relocations\":[", object.readonly).unwrap();
            if let Some(callable) = object.code {
                write!(out, "{{\"addend\":0,\"address\":\"far\",\"at\":0,\"code\":true,\"target\":{callable}}}").unwrap();
            }
            out.push_str("]}");
        }
        out.push_str("],\"functions\":[");
        for (index, function) in self.functions.iter().enumerate() {
            comma(&mut out, index);
            function_json(&mut out, function);
        }
        out.push_str("],\"id\":1,\"name\":");
        string(&mut out, &self.module_name);
        let mut stated = llrm_core::hir::facts::Builder::new("nib");
        for function in &self.functions {
            stated.extend(function.facts.iter().cloned());
            // A reference's place stays inside what it refers to, where the language checked it.
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                // A bool is 0 or 1, whoever stored it.
                let loaded = instruction.op == "load" && instruction.results.first().is_some_and(|result| {
                    function.values.iter().find(|one| one.id == *result).and_then(|one| self.types.iter().find(|ty| ty.id == one.type_id)).is_some_and(|ty| ty.kind == "boolean")
                });
                if loaded {
                    stated.state(llrm_core::hir::facts::Subject::Instruction { function: i64::from(function.id), id: i64::from(instruction.id) }, llrm_mir::facts::Fact::Range(llrm_mir::facts::Bounds { lo: 0, hi: 1 }));
                }
                if instruction.inbounds {
                    stated.state(llrm_core::hir::facts::Subject::Instruction { function: i64::from(function.id), id: i64::from(instruction.id) }, llrm_mir::facts::Fact::InBounds);
                }
                for (index, operand) in instruction.operands.iter().enumerate() {
                    if matches!(operand, Operand::IndirectPlace { inbounds: true, .. }) {
                        stated.state(
                            llrm_core::hir::facts::Subject::Operand { function: i64::from(function.id), instruction: i64::from(instruction.id), operand: index as i64 },
                            llrm_mir::facts::Fact::InBounds,
                        );
                    }
                }
            }
        }
        stated.extend(self.facts.iter().cloned());
        let facts = stated.finish();
        if !facts.is_empty() {
            out.push_str(",\"facts\":");
            out.push_str(&llrm_core::hir::codec::facts_json(&facts));
        }
        if let Some(debug) = &self.debug {
            out.push_str(",\"debug\":");
            out.push_str(&llrm_core::hir::codec::debug_json(debug));
        }
        out.push_str(",\"types\":[");
        for (index, type_) in self.types.iter().enumerate() {
            comma(&mut out, index);
            write!(out, "{{\"address\":\"{}\",\"bounds\":[", type_.address).unwrap();
            for (bound_index, (lower, upper)) in type_.bounds.iter().enumerate() {
                comma(&mut out, bound_index);
                write!(out, "[{lower},{upper}]").unwrap();
            }
            out.push_str("],\"element\":");
            optional_number(&mut out, type_.element);
            write!(
                out,
                ",\"evaluation\":\"{}\",\"id\":{},\"kind\":\"{}\",\"name\":",
                type_.evaluation, type_.id, type_.kind
            )
            .unwrap();
            string(&mut out, &type_.name);
            write!(out, ",\"rank\":{},\"signed\":", type_.rank).unwrap();
            match type_.signed {
                Some(value) => out.push_str(if value { "true" } else { "false" }),
                None => out.push_str("null"),
            }
            write!(out, ",\"width\":{}}}", type_.width).unwrap();
        }
        out.push_str(
            "]}],\"runtime\":\"freestanding\",\"schema\":5,\"target\":\"i386-real-mode\",\"zeroed_locals\":false}\n",
        );
        out
    }
}

fn function_json(out: &mut String, function: &Function) {
    match &function.abi {
        Some(ProcedureAbi { distance, cleanup, parameter_bytes, float_return }) => write!(
            out,
            "{{\"abi\":{{\"cleanup\":\"{cleanup}\",\"distance\":\"{distance}\",\"float_return\":\"{float_return}\",\"parameter_bytes\":{parameter_bytes}}},\"blocks\":["
        )
        .unwrap(),
        None => out.push_str("{\"abi\":null,\"blocks\":["),
    }
    for (index, block) in function.blocks.iter().enumerate() {
        comma(out, index);
        write!(out, "{{\"id\":{},\"instructions\":[", block.id).unwrap();
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            comma(out, instruction_index);
            out.push('{');
            if let Some(asm) = &instruction.asm {
                out.push_str("\"asm\":{\"clobbers\":[");
                strings(out, &asm.clobbers);
                out.push_str("],\"code\":[");
                bytes(out, &asm.code);
                out.push_str("],\"inputs\":[");
                strings(out, &asm.inputs);
                write!(out, "],\"memory\":{},\"outputs\":[", asm.memory).unwrap();
                strings(out, &asm.outputs);
                out.push_str("]},");
            }
            out.push_str("\"callee\":");
            if let Some(callee) = &instruction.callee {
                string(out, callee);
            } else {
                out.push_str("null");
            }
            write!(
                out,
                ",\"id\":{},\"op\":\"{}\",\"operands\":[",
                instruction.id, instruction.op
            )
            .unwrap();
            operands(out, &instruction.operands);
            out.push_str("],\"pure\":false,\"results\":[");
            numbers(out, &instruction.results);
            out.push(']');
            if instruction.line > 0 {
                write!(out, ",\"line\":{}", instruction.line).unwrap();
            }
            out.push('}');
        }
        out.push_str("],\"terminator\":{\"cases\":[],\"kind\":");
        string(out, block.terminator.kind);
        out.push_str(",\"operands\":[");
        operands(out, &block.terminator.operands);
        out.push_str("],\"targets\":[");
        numbers(out, &block.terminator.targets);
        out.push_str("]}}");
    }
    out.push_str("],\"calls\":[");
    for (index, call) in function.calls.iter().enumerate() {
        comma(out, index);
        write!(
            out,
            "{{\"callee\":{},\"cleanup\":\"{}\",\"distance\":\"far\",\"float_return\":\"{}\",\"instruction\":{},\"order\":[",
            call.callee,
            if call.callee_cleans { "callee" } else { "caller" },
            call.float_return,
            call.instruction
        )
        .unwrap();
        numbers(out, &call.order);
        out.push_str("]}");
    }
    write!(
        out,
        "],\"entry\":{},\"error_handler\":null,\"error_handler_local\":false,\"external_entries\":[],\"id\":{},\"linkage\":\"{}\",\"name\":",
        function.entry,
        function.id,
        if function.exported { "external" } else { "internal" }
    )
    .unwrap();
    string(out, &function.name);
    out.push_str(",\"parameters\":[");
    numbers(out, &function.parameters);
    out.push_str("],\"places\":[");
    for (index, place) in function.places.iter().enumerate() {
        comma(out, index);
        write!(
            out,
            "{{\"address\":\"near\",\"extent\":{},\"id\":{},\"name\":",
            place.extent, place.id
        )
        .unwrap();
        string(out, &place.name);
        write!(
            out,
            ",\"offset\":{},\"storage\":\"{}\",\"symbol\":{},\"type\":{},\"volatile\":{}}}",
            place.offset, place.storage, place.symbol, place.type_id, place.volatile
        )
        .unwrap();
    }
    write!(
        out,
        "],\"result_type\":{},\"values\":[",
        function.result_type
    )
    .unwrap();
    for (index, value) in function.values.iter().enumerate() {
        comma(out, index);
        write!(out, "{{\"id\":{},\"type\":{}}}", value.id, value.type_id).unwrap();
    }
    out.push_str("]}");
}

fn operands(out: &mut String, values: &[Operand]) {
    for (index, operand) in values.iter().enumerate() {
        comma(out, index);
        match operand {
            Operand::Value(value) => {
                write!(out, "{{\"tag\":\"value\",\"value\":{value}}}").unwrap()
            }
            Operand::Constant(type_id, value) => write!(
                out,
                "{{\"tag\":\"constant\",\"type\":{type_id},\"value\":{value}}}"
            )
            .unwrap(),
            Operand::Place(place) => {
                write!(out, "{{\"place\":{place},\"tag\":\"place\"}}").unwrap()
            }
            Operand::ArrayElement(place, indices) => {
                write!(out, "{{\"indices\":[").unwrap();
                operands(out, indices);
                write!(out, "],\"place\":{place},\"tag\":\"array_element\"}}").unwrap();
            }
            Operand::ProjectedPlace {
                place,
                indices,
                offset,
                type_id,
                member,
            } => {
                write!(out, "{{\"indices\":[").unwrap();
                operands(out, indices);
                write!(out, "],").unwrap();
                if let Some((owner, at)) = member {
                    write!(out, "\"member\":{{\"offset\":{at},\"owner\":{owner}}},").unwrap();
                }
                write!(
                    out,
                    "\"offset\":{offset},\"place\":{place},\"tag\":\"projection\",\"type\":{type_id}}}"
                )
                .unwrap();
            }
            Operand::IndirectPlace {
                base,
                offset,
                type_id,
                inbounds: _,
                member,
            } => {
                write!(out, "{{\"base\":{base},").unwrap();
                if let Some((owner, at)) = member {
                    write!(out, "\"member\":{{\"offset\":{at},\"owner\":{owner}}},").unwrap();
                }
                write!(
                    out,
                    "\"offset\":{offset},\"tag\":\"indirect\",\"type\":{type_id},\"volatile\":false}}"
                )
                .unwrap()
            }
            Operand::DescriptorPlace {
                base,
                field,
                type_id,
            } => write!(
                out,
                "{{\"base\":{base},\"field\":\"{field}\",\"tag\":\"descriptor\",\"type\":{type_id}}}"
            )
            .unwrap(),
        }
    }
}

fn numbers(out: &mut String, values: &[u32]) {
    for (index, value) in values.iter().enumerate() {
        comma(out, index);
        write!(out, "{value}").unwrap();
    }
}

fn strings(out: &mut String, values: &[String]) {
    for (index, value) in values.iter().enumerate() {
        comma(out, index);
        string(out, value);
    }
}

fn bytes(out: &mut String, values: &[u8]) {
    for (index, value) in values.iter().enumerate() {
        comma(out, index);
        write!(out, "{value}").unwrap();
    }
}

fn booleans(out: &mut String, count: usize, value: bool) {
    for index in 0..count {
        comma(out, index);
        out.push_str(if value { "true" } else { "false" });
    }
}

fn optional_number(out: &mut String, value: Option<u32>) {
    if let Some(value) = value {
        write!(out, "{value}").unwrap();
    } else {
        out.push_str("null");
    }
}

fn comma(out: &mut String, index: usize) {
    if index != 0 {
        out.push(',');
    }
}

fn string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            one if one.is_control() => write!(out, "\\u{:04x}", one as u32).unwrap(),
            one => out.push(one),
        }
    }
    out.push('"');
}
