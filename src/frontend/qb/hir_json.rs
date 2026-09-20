//! Transitional encoder for the Python-era QB HIR JSON format.
//!
//! The format is intentionally kept at the frontend edge. New Rust pipeline
//! code consumes [`crate::hir::Program`] directly; this encoder exists only
//! until the legacy JSON consumers have been retired.

use std::fmt::{self, Write};

use crate::hir::{
    ArrayOrder, CallDistance, ConstantValue, Dialect, FloatMode, Operand, Program, RuntimeProfile,
    StackCleanup, TargetProfile, Terminator,
};

/// Serializes a typed HIR program in the exact deterministic JSON layout used
/// by the legacy QB frontend.
pub(crate) fn write(program: &Program) -> Result<String, Error> {
    if program
        .modules
        .iter()
        .flat_map(|module| &module.functions)
        .flat_map(|function| &function.blocks)
        .any(|block| matches!(block.terminator, Terminator::Switch { .. }))
    {
        return Err(Error::UnsupportedSwitch);
    }

    let mut out = String::new();
    write!(
        out,
        "{{\"dialect\":\"{}\",\"modules\":[",
        dialect_name(program.dialect)
    )
    .unwrap();

    for (module_index, module) in program.modules.iter().enumerate() {
        if module_index != 0 {
            out.push(',');
        }
        out.push_str("{\"functions\":[");
        for (function_index, function) in module.functions.iter().enumerate() {
            if function_index != 0 {
                out.push(',');
            }
            out.push_str("{\"blocks\":[");
            for (block_index, block) in function.blocks.iter().enumerate() {
                if block_index != 0 {
                    out.push(',');
                }
                write!(out, "{{\"id\":{},\"instructions\":[", block.id).unwrap();
                for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                    if instruction_index != 0 {
                        out.push(',');
                    }
                    out.push_str("{\"callee\":");
                    option_string(&mut out, instruction.callee.as_deref());
                    write!(
                        out,
                        ",\"id\":{},\"op\":\"{}\",\"operands\":[",
                        instruction.id,
                        instruction.opcode.as_str()
                    )
                    .unwrap();
                    operands(&mut out, &instruction.operands);
                    out.push_str("],\"pure\":false,\"results\":[");
                    value_ids(&mut out, &instruction.results);
                    out.push_str("]}");
                }
                out.push_str("],\"terminator\":{\"cases\":[],\"kind\":");
                terminator_json(&mut out, &block.terminator);
                out.push('}');
            }
            write!(
                out,
                "],\"abi\":{{\"cleanup\":\"{}\",\"distance\":\"{}\",\"parameter_bytes\":{}}},\"calls\":[",
                cleanup_name(function.abi.cleanup),
                distance_name(function.abi.distance),
                function.abi.parameter_bytes
            )
            .unwrap();
            for (call_index, call) in function.calls.iter().enumerate() {
                if call_index != 0 {
                    out.push(',');
                }
                out.push_str("{\"callee\":");
                option_id(&mut out, call.callee.map(|id| id.get()));
                write!(
                    out,
                    ",\"cleanup\":\"{}\",\"distance\":\"{}\",\"instruction\":{},\"order\":[",
                    cleanup_name(call.cleanup),
                    distance_name(call.distance),
                    call.instruction
                )
                .unwrap();
                usize_values(&mut out, &call.order);
                out.push_str("]}");
            }
            write!(out, "],\"entry\":{},\"error_handler\":", function.entry).unwrap();
            option_id(&mut out, function.error_handler.map(|id| id.get()));
            write!(
                out,
                ",\"error_handler_local\":{}",
                function.error_handler_local
            )
            .unwrap();
            out.push_str(",\"external_entries\":[");
            block_ids(&mut out, &function.external_entries);
            write!(out, "],\"id\":{},\"name\":", function.id).unwrap();
            string(&mut out, &function.name);
            write!(out, ",\"linkage\":\"{}\"", function.linkage.as_str()).unwrap();
            out.push_str(",\"parameters\":[");
            value_ids(&mut out, &function.parameters);
            out.push_str("],\"places\":[");
            for (place_index, place) in function.places.iter().enumerate() {
                if place_index != 0 {
                    out.push(',');
                }
                write!(
                    out,
                    "{{\"address\":\"{}\",\"extent\":{},\"id\":{},\"name\":",
                    place.address.as_str(),
                    place.extent,
                    place.id
                )
                .unwrap();
                string(&mut out, &place.name);
                write!(
                    out,
                    ",\"offset\":{},\"storage\":\"{}\",\"symbol\":{},\"type\":{}}}",
                    place.offset,
                    place.storage.as_str(),
                    place.symbol,
                    place.type_id
                )
                .unwrap();
            }
            write!(
                out,
                "],\"result_type\":{},\"values\":[",
                function.result_type
            )
            .unwrap();
            for (value_index, value) in function.values.iter().enumerate() {
                if value_index != 0 {
                    out.push(',');
                }
                write!(out, "{{\"id\":{},\"type\":{}}}", value.id, value.type_id).unwrap();
            }
            out.push_str("]}");
        }
        out.push_str("],\"callables\":[");
        for (callable_index, callable) in module.callables.iter().enumerate() {
            if callable_index != 0 {
                out.push(',');
            }
            out.push_str("{\"arrays\":[");
            booleans(
                &mut out,
                callable.parameters.iter().map(|parameter| parameter.array),
            );
            out.push_str("],\"by_value\":[");
            booleans(
                &mut out,
                callable
                    .parameters
                    .iter()
                    .map(|parameter| parameter.by_value),
            );
            write!(
                out,
                "],\"defined\":{},\"id\":{},\"name\":",
                callable.defined, callable.id
            )
            .unwrap();
            string(&mut out, &callable.name);
            out.push_str(",\"parameter_types\":[");
            for (parameter_index, parameter) in callable.parameters.iter().enumerate() {
                if parameter_index != 0 {
                    out.push(',');
                }
                write!(out, "{}", parameter.type_id).unwrap();
            }
            out.push_str("],\"result_type\":");
            option_id(&mut out, callable.result_type.map(|id| id.get()));
            out.push_str(",\"segmented\":[");
            booleans(
                &mut out,
                callable
                    .parameters
                    .iter()
                    .map(|parameter| parameter.segmented),
            );
            out.push_str("]}");
        }
        out.push_str("],\"data\":[");
        for (data_index, data) in module.data.iter().enumerate() {
            if data_index != 0 {
                out.push(',');
            }
            out.push_str("{\"bytes\":[");
            byte_values(&mut out, &data.bytes);
            write!(
                out,
                "],\"address\":\"{}\",\"id\":{},\"linkage\":\"{}\",\"name\":",
                data.address.as_str(),
                data.id,
                data.linkage.as_str()
            )
            .unwrap();
            string(&mut out, &data.name);
            write!(out, ",\"readonly\":{},\"relocations\":[", data.readonly).unwrap();
            for (relocation_index, relocation) in data.relocations.iter().enumerate() {
                if relocation_index != 0 {
                    out.push(',');
                }
                write!(
                    out,
                    "{{\"addend\":{},\"address\":\"{}\",\"at\":{},\"target\":{}}}",
                    relocation.addend,
                    relocation.address.as_str(),
                    relocation.at,
                    relocation.target
                )
                .unwrap();
            }
            out.push_str("]}");
        }
        write!(out, "],\"id\":{},\"name\":", module.id).unwrap();
        string(&mut out, &module.name);
        out.push_str(",\"types\":[");
        for (type_index, type_) in module.types.iter().enumerate() {
            if type_index != 0 {
                out.push(',');
            }
            type_json(&mut out, type_);
        }
        out.push_str("]}");
    }
    write!(
        out,
        "],\"runtime\":\"{}\",\"schema\":{},\"target\":\"{}\",\"array_order\":\"{}\",\"float_mode\":\"{}\"}}\n",
        runtime_name(program.runtime),
        program.version,
        target_name(program.target),
        array_order_name(program.array_order),
        float_mode_name(program.float_mode)
    )
    .unwrap();
    Ok(out)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Error {
    UnsupportedSwitch,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedSwitch => {
                formatter.write_str("legacy QB HIR JSON cannot represent switch case values")
            }
        }
    }
}

impl std::error::Error for Error {}

fn terminator_json(out: &mut String, terminator: &Terminator) {
    match terminator {
        Terminator::Jump(target) => {
            out.push_str("\"jump\",\"operands\":[],\"targets\":[");
            write!(out, "{target}").unwrap();
        }
        Terminator::Branch {
            condition,
            then_block,
            else_block,
        } => {
            out.push_str("\"branch\",\"operands\":[");
            operand_json(out, condition);
            out.push_str("],\"targets\":[");
            write!(out, "{then_block},{else_block}").unwrap();
        }
        Terminator::Switch {
            selector: _,
            cases: _,
            default: _,
        } => {
            unreachable!("switch terminators are rejected before serialization")
        }
        Terminator::Return(value) => {
            out.push_str("\"return\",\"operands\":[");
            if let Some(value) = value {
                operand_json(out, value);
            }
            out.push_str("],\"targets\":[");
        }
        Terminator::Unreachable => {
            out.push_str("\"unreachable\",\"operands\":[],\"targets\":[");
        }
    }
    out.push_str("]}");
}

fn operands(out: &mut String, operands: &[Operand]) {
    for (operand_index, operand) in operands.iter().enumerate() {
        if operand_index != 0 {
            out.push(',');
        }
        operand_json(out, operand);
    }
}

fn operand_json(out: &mut String, operand: &Operand) {
    match operand {
        Operand::Value(value) => write!(out, "{{\"tag\":\"value\",\"value\":{value}}}").unwrap(),
        Operand::Constant {
            type_id,
            value: ConstantValue::Integer(value),
        } => write!(
            out,
            "{{\"tag\":\"constant\",\"type\":{type_id},\"value\":{value}}}"
        )
        .unwrap(),
        Operand::Constant {
            type_id,
            value: ConstantValue::Real(value),
        } => write!(
            out,
            "{{\"tag\":\"constant\",\"type\":{type_id},\"value\":{value}}}"
        )
        .unwrap(),
        Operand::Place(place) => write!(out, "{{\"place\":{place},\"tag\":\"place\"}}").unwrap(),
        Operand::Element { place, indices } => {
            out.push_str("{\"indices\":[");
            operands(out, indices);
            write!(out, "],\"place\":{place},\"tag\":\"array_element\"}}").unwrap();
        }
        Operand::Projection {
            place,
            indices,
            offset,
            type_id,
        } => {
            out.push_str("{\"indices\":[");
            operands(out, indices);
            write!(
                out,
                "],\"offset\":{offset},\"place\":{place},\"tag\":\"projection\",\"type\":{type_id}}}"
            )
            .unwrap();
        }
        Operand::Indirect {
            base,
            offset,
            type_id,
            volatile,
        } => write!(
            out,
            "{{\"base\":{base},\"offset\":{offset},\"tag\":\"indirect\",\"type\":{type_id},\"volatile\":{volatile}}}"
        )
        .unwrap(),
    }
}

fn type_json(out: &mut String, type_: &crate::hir::Type) {
    write!(
        out,
        "{{\"address\":\"{}\",\"bounds\":[",
        type_.address.as_str()
    )
    .unwrap();
    for (bound_index, (lower, upper)) in type_.bounds.iter().enumerate() {
        if bound_index != 0 {
            out.push(',');
        }
        write!(out, "[{lower},{upper}]").unwrap();
    }
    out.push_str("],\"element\":");
    option_id(out, type_.element.map(|id| id.get()));
    write!(
        out,
        ",\"evaluation\":\"{}\",\"id\":{},\"kind\":\"{}\",\"name\":",
        type_.evaluation.as_str(),
        type_.id,
        type_.kind.as_str()
    )
    .unwrap();
    string(out, &type_.name);
    write!(out, ",\"rank\":{},\"signed\":", type_.bounds.len()).unwrap();
    boolean_option(out, type_.signed);
    write!(out, ",\"width\":{}}}", type_.width).unwrap();
}

fn string(out: &mut String, value: &str) {
    out.push('"');
    for character in value.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            character => out.push(character),
        }
    }
    out.push('"');
}

fn option_string(out: &mut String, value: Option<&str>) {
    match value {
        Some(value) => string(out, value),
        None => out.push_str("null"),
    }
}

fn option_id(out: &mut String, value: Option<u32>) {
    match value {
        Some(value) => write!(out, "{value}").unwrap(),
        None => out.push_str("null"),
    }
}

fn boolean_option(out: &mut String, value: Option<bool>) {
    match value {
        Some(value) => out.push_str(if value { "true" } else { "false" }),
        None => out.push_str("null"),
    }
}

fn booleans(out: &mut String, values: impl Iterator<Item = bool>) {
    for (value_index, value) in values.enumerate() {
        if value_index != 0 {
            out.push(',');
        }
        out.push_str(if value { "true" } else { "false" });
    }
}

fn byte_values(out: &mut String, values: &[u8]) {
    for (value_index, value) in values.iter().enumerate() {
        if value_index != 0 {
            out.push(',');
        }
        write!(out, "{value}").unwrap();
    }
}

fn usize_values(out: &mut String, values: &[usize]) {
    for (value_index, value) in values.iter().enumerate() {
        if value_index != 0 {
            out.push(',');
        }
        write!(out, "{value}").unwrap();
    }
}

fn value_ids(out: &mut String, values: &[crate::hir::ValueId]) {
    for (value_index, value) in values.iter().enumerate() {
        if value_index != 0 {
            out.push(',');
        }
        write!(out, "{value}").unwrap();
    }
}

fn block_ids(out: &mut String, values: &[crate::hir::BlockId]) {
    for (value_index, value) in values.iter().enumerate() {
        if value_index != 0 {
            out.push(',');
        }
        write!(out, "{value}").unwrap();
    }
}

fn dialect_name(dialect: Dialect) -> &'static str {
    match dialect {
        Dialect::Qbasic11 => "qbasic11",
        Dialect::Qb45 => "qb45",
        Dialect::Pds71 => "pds71",
        Dialect::Vbdos => "vbdos",
    }
}

fn runtime_name(runtime: RuntimeProfile) -> &'static str {
    match runtime {
        RuntimeProfile::Qb45 => "qb45",
        RuntimeProfile::Pds71 => "pds71",
        RuntimeProfile::Vbdos => "vbdos",
    }
}

fn target_name(target: TargetProfile) -> &'static str {
    match target {
        TargetProfile::I386RealMode => "i386-real-mode",
    }
}

fn array_order_name(array_order: ArrayOrder) -> &'static str {
    match array_order {
        ArrayOrder::ColumnMajor => "column-major",
        ArrayOrder::RowMajor => "row-major",
    }
}

fn float_mode_name(float_mode: FloatMode) -> &'static str {
    match float_mode {
        FloatMode::Inline => "inline",
        FloatMode::Alternate => "alternate",
    }
}

fn cleanup_name(cleanup: StackCleanup) -> &'static str {
    match cleanup {
        StackCleanup::Caller => "caller",
        StackCleanup::Callee => "callee",
    }
}

fn distance_name(distance: CallDistance) -> &'static str {
    match distance {
        CallDistance::Near => "near",
        CallDistance::Far => "far",
    }
}
