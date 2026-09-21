use std::fmt::Write;

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DataObject {
    pub id: u32,
    pub name: String,
    pub bytes: Vec<u8>,
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
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Instruction {
    pub id: u32,
    pub op: &'static str,
    pub results: Vec<u32>,
    pub operands: Vec<Operand>,
    pub callee: Option<String>,
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
    pub data: Vec<DataObject>,
}

impl Program {
    /// The common HIR wire format is intentionally written without a serde
    /// dependency so the standalone frontend remains one small binary.
    pub fn json(&self) -> String {
        let mut out = String::new();
        out.push_str("{\"array_order\":\"column-major\",\"dialect\":\"modern\",\"float_mode\":\"inline\",\"modules\":[{");
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
            write!(out, "{{\"address\":\"near\",\"bytes\":[").unwrap();
            bytes(&mut out, &object.bytes);
            write!(
                out,
                "],\"id\":{},\"linkage\":\"internal\",\"name\":",
                object.id
            )
            .unwrap();
            string(&mut out, &object.name);
            out.push_str(",\"readonly\":true,\"relocations\":[]}");
        }
        out.push_str("],\"functions\":[");
        for (index, function) in self.functions.iter().enumerate() {
            comma(&mut out, index);
            function_json(&mut out, function);
        }
        out.push_str("],\"id\":1,\"name\":");
        string(&mut out, &self.module_name);
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
            "]}],\"runtime\":\"freestanding\",\"schema\":1,\"target\":\"i386-real-mode\"}\n",
        );
        out
    }
}

fn function_json(out: &mut String, function: &Function) {
    out.push_str("{\"abi\":null,\"blocks\":[");
    for (index, block) in function.blocks.iter().enumerate() {
        comma(out, index);
        write!(out, "{{\"id\":{},\"instructions\":[", block.id).unwrap();
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            comma(out, instruction_index);
            out.push_str("{\"callee\":");
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
            out.push_str("]}");
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
            "{{\"callee\":{},\"cleanup\":\"caller\",\"distance\":\"far\",\"instruction\":{},\"order\":[",
            call.callee, call.instruction
        )
        .unwrap();
        numbers(out, &call.order);
        out.push_str("]}");
    }
    write!(
        out,
        "],\"entry\":{},\"error_handler\":null,\"error_handler_local\":false,\"external_entries\":[],\"id\":{},\"linkage\":\"internal\",\"name\":",
        function.entry, function.id
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
            ",\"offset\":{},\"storage\":\"{}\",\"symbol\":{},\"type\":{}}}",
            place.offset, place.storage, place.symbol, place.type_id
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
            } => {
                write!(out, "{{\"indices\":[").unwrap();
                operands(out, indices);
                write!(
                    out,
                    "],\"offset\":{offset},\"place\":{place},\"tag\":\"projection\",\"type\":{type_id}}}"
                )
                .unwrap();
            }
        }
    }
}

fn numbers(out: &mut String, values: &[u32]) {
    for (index, value) in values.iter().enumerate() {
        comma(out, index);
        write!(out, "{value}").unwrap();
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
