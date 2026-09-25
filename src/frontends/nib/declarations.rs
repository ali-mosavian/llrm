//! `.H`, `.BI` and `.INC` declarations of a module's exports (section 15),
//! so C, BASIC and assembler callers link against them. They are tooling
//! outputs, printed from the exports' source signatures.

use std::fmt::Write;

use super::error::Diagnostic;
use super::standard;
use super::syntax::{Abi, Adapter, Function, Module, ParameterType, Span, Struct, TypeAnnotation, TypeName, TypeSpec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Language {
    C,
    Basic,
    Assembler,
}

impl Language {
    pub fn named(extension: &str) -> Option<Self> {
        match extension.to_ascii_lowercase().as_str() {
            "h" => Some(Self::C),
            "bi" => Some(Self::Basic),
            "inc" => Some(Self::Assembler),
            _ => None,
        }
    }
}

/// The declarations of `module`'s exports and the represented structs they
/// name, for callers written in `language`.
pub fn declarations(module: &Module, name: &str, language: Language) -> Result<String, Diagnostic> {
    let exports: Vec<(&Function, Abi)> = module
        .functions
        .iter()
        .filter_map(|function| module.exports.get(&function.name).map(|export| (function, export.abi)))
        .collect();
    // The compiler's own modules declare theirs for the compiler.
    let structs: Vec<&Struct> = module.structs.iter().filter(|one| one.pack.is_some() && !standard::supplied(&one.name)).collect();
    let mut out = String::new();
    let comment = match language {
        Language::C => "/*",
        Language::Basic => "'",
        Language::Assembler => ";",
    };
    let close = if language == Language::C { " */" } else { "" };
    writeln!(out, "{comment} Declarations of {name}.nbl's exports. Generated; do not edit.{close}").unwrap();
    let guard = format!("{}_H", name.to_ascii_uppercase().replace(['.', '-'], "_"));
    if language == Language::C {
        writeln!(out, "#ifndef {guard}\n#define {guard}").unwrap();
    }
    for one in &structs {
        out.push('\n');
        out.push_str(&structure(one, language)?);
    }
    if !exports.is_empty() {
        out.push('\n');
    }
    for (function, abi) in exports {
        writeln!(out, "{}", declaration(function, abi, language)?).unwrap();
    }
    if language == Language::C {
        writeln!(out, "\n#endif").unwrap();
    }
    Ok(out)
}

fn structure(one: &Struct, language: Language) -> Result<String, Diagnostic> {
    let name = symbol(&one.name);
    let mut out = String::new();
    match language {
        Language::C => {
            writeln!(out, "#pragma pack({})\ntypedef struct {{", one.pack.expect("represented")).unwrap();
            for field in &one.fields {
                let dims: String = field.dims.iter().map(|dim| format!("[{dim}]")).collect();
                writeln!(out, "    {}{dims};", c_declarator(&field.type_spec, &field.name, one.span)?).unwrap();
            }
            writeln!(out, "}} {name};\n#pragma pack()").unwrap();
        }
        Language::Basic => {
            writeln!(out, "TYPE {name}").unwrap();
            for field in &one.fields {
                // A TYPE holds no array.
                let type_ = basic_field(&field.type_spec)
                    .filter(|_| field.dims.is_empty())
                    .ok_or_else(|| unsupported(&field.name, "BASIC", field.span))?;
                writeln!(out, "    {} AS {type_}", field.name).unwrap();
            }
            writeln!(out, "END TYPE").unwrap();
        }
        Language::Assembler => {
            writeln!(out, "{name} struct").unwrap();
            for field in &one.fields {
                let (type_, initial) = match &field.type_spec {
                    TypeSpec::Named(inner) => (symbol(inner), "<>"),
                    spec => (data_directive(width(spec).ok_or_else(|| unsupported(&field.name, "assembler", field.span))?).to_owned(), "?"),
                };
                let count: u32 = field.dims.iter().product();
                let directive = if field.dims.is_empty() { format!("{type_} {initial}") } else { format!("{type_} {count} dup ({initial})") };
                writeln!(out, "    {} {directive}", field.name).unwrap();
            }
            writeln!(out, "{name} ends").unwrap();
        }
    }
    Ok(out)
}

fn declaration(function: &Function, abi: Abi, language: Language) -> Result<String, Diagnostic> {
    // The name its source gives it, not the one it is linked under.
    let name = function.name.rsplit('.').next().expect("a name");
    let parameters: Vec<(&str, &TypeSpec)> = function
        .parameters
        .iter()
        .map(|parameter| match &parameter.type_ {
            ParameterType::Owned(TypeAnnotation::Value(spec)) => Ok((parameter.name.as_str(), spec)),
            _ => Err(unsupported(&parameter.name, "a foreign ABI", parameter.span)),
        })
        .collect::<Result<_, _>>()?;
    // A `&string` result, BASIC's string function.
    let text = TypeSpec::Primitive(TypeName::String);
    let result = match &function.result {
        TypeAnnotation::Value(result) => result,
        TypeAnnotation::Slice { element: TypeSpec::Primitive(TypeName::Char), rank: 1 } if abi.basic().is_some() => &text,
        _ => return Err(unsupported(&function.name, "a foreign ABI", function.span)),
    };
    let span = function.span;
    // A BASIC float result's hidden pointer is one more word.
    let hidden = abi.basic().is_some() && matches!(result, TypeSpec::Primitive(TypeName::F32 | TypeName::F64));
    match language {
        Language::C => {
            if abi.basic().is_some() {
                return Err(unsupported(&function.name, "C; BASIC calls it", span));
            }
            let convention = match abi {
                Abi::Cdecl16 => "__cdecl",
                Abi::Interrupt16 => "__interrupt",
                Abi::Pascal16 | Abi::Basic(_) => "__pascal",
            };
            let arguments = parameters
                .iter()
                .map(|(name, spec)| c_declarator(spec, name, span))
                .collect::<Result<Vec<_>, _>>()?;
            let arguments = if arguments.is_empty() { "void".to_owned() } else { arguments.join(", ") };
            Ok(format!("extern {} __far {convention} {}({arguments});", c_type(result, span)?, name))
        }
        // BASIC cannot name a handler's address: there is nothing to declare.
        Language::Basic if abi.interrupt() => Ok(format!("' {name}: an interrupt16 handler")),
        Language::Basic => {
            let arguments = parameters
                .iter()
                .map(|(name, spec)| basic_parameter(name, spec).ok_or_else(|| unsupported(name, "BASIC", span)))
                .collect::<Result<Vec<_>, _>>()?;
            let convention = if abi == Abi::Cdecl16 { " CDECL" } else { "" };
            // BASIC names hold letters, digits and periods; any other takes its symbol as an alias.
            let (name, alias) = if name.chars().all(|one| one.is_ascii_alphanumeric() || one == '.') {
                (name.to_owned(), String::new())
            } else {
                (name.replace('_', ""), format!(" ALIAS \"{}\"", abi.symbol(name)))
            };
            let arguments = if arguments.is_empty() { String::new() } else { format!(" ({})", arguments.join(", ")) };
            let head = match result {
                TypeSpec::Primitive(TypeName::Void) => format!("SUB {name}"),
                spec => {
                    let suffix = basic_suffix(spec, abi).ok_or_else(|| unsupported(&function.name, "BASIC", span))?;
                    format!("FUNCTION {name}{suffix}")
                }
            };
            Ok(format!("DECLARE {head}{convention}{alias}{arguments}"))
        }
        Language::Assembler => {
            let words: u32 = parameters.iter().map(|(_, spec)| argument_width(spec)).sum::<u32>() + if hidden { 2 } else { 0 };
            let signature = parameters
                .iter()
                .map(|(name, spec)| format!("{name}: {}", spec.text()))
                .collect::<Vec<_>>()
                .join(", ");
            let cleanup = match abi {
                Abi::Cdecl16 => "caller removes the arguments".to_owned(),
                Abi::Interrupt16 => "iret".to_owned(),
                _ => format!("retf {words}"),
            };
            Ok(format!(
                "extrn {}:far    ; {}({signature}) -> {}, {cleanup}",
                abi.symbol(name),
                abi.name(),
                result.text()
            ))
        }
    }
}

/// The bytes an argument of `spec` is pushed as: a BASIC adapter is a near pointer.
fn argument_width(spec: &TypeSpec) -> u32 {
    if Adapter::of(spec).is_some() {
        return 2;
    }
    width(spec).unwrap_or(2).max(2)
}

fn unsupported(name: &str, what: &str, span: Span) -> Diagnostic {
    Diagnostic::new(span, format!("{name:?} has no declaration in {what}"))
}

/// A struct name as a symbol: a module's `a.Point` is `a_Point`.
fn symbol(name: &str) -> String {
    name.replace('.', "_")
}

/// A raw pointer's distance, mutability and target.
fn pointer(spec: &TypeSpec) -> Option<(bool, bool, &TypeSpec)> {
    let TypeSpec::Applied { name, args } = spec else {
        return None;
    };
    let [TypeAnnotation::Value(target)] = args.as_slice() else {
        return None;
    };
    let far = name.starts_with("*far");
    (far || name.starts_with("*near")).then_some((far, name.ends_with(" mut"), target))
}

fn width(spec: &TypeSpec) -> Option<u32> {
    if let Some((far, _, _)) = pointer(spec) {
        return Some(if far { 4 } else { 2 });
    }
    match spec {
        TypeSpec::Primitive(TypeName::I8 | TypeName::U8 | TypeName::Bool | TypeName::Char) => Some(1),
        TypeSpec::Primitive(TypeName::I16 | TypeName::U16) => Some(2),
        TypeSpec::Primitive(TypeName::I32 | TypeName::U32 | TypeName::F32) => Some(4),
        TypeSpec::Primitive(TypeName::F64) => Some(8),
        _ => None,
    }
}

fn data_directive(width: u32) -> &'static str {
    match width {
        1 => "db",
        2 => "dw",
        4 => "dd",
        _ => "dq",
    }
}

fn c_type(spec: &TypeSpec, span: Span) -> Result<String, Diagnostic> {
    if let Some((far, mutable, target)) = pointer(spec) {
        let constant = if mutable { "" } else { "const " };
        let distance = if far { "__far" } else { "__near" };
        return Ok(format!("{constant}{} {distance} *", c_type(target, span)?));
    }
    Ok(match spec {
        TypeSpec::Primitive(type_name) => match type_name {
            TypeName::I8 => "signed char",
            TypeName::U8 | TypeName::Bool => "unsigned char",
            TypeName::Char => "char",
            TypeName::I16 => "short",
            TypeName::U16 => "unsigned short",
            TypeName::I32 => "long",
            TypeName::U32 => "unsigned long",
            TypeName::F32 => "float",
            TypeName::F64 => "double",
            TypeName::Void => "void",
            _ => return Err(unsupported(&spec.text(), "C", span)),
        }
        .to_owned(),
        TypeSpec::Named(name) => symbol(name),
        TypeSpec::Applied { .. } => return Err(unsupported(&spec.text(), "C", span)),
    })
}

fn c_declarator(spec: &TypeSpec, name: &str, span: Span) -> Result<String, Diagnostic> {
    let type_ = c_type(spec, span)?;
    Ok(if type_.ends_with('*') { format!("{type_}{name}") } else { format!("{type_} {name}") })
}

/// A BASIC type a value of `spec` is, where BASIC has one.
fn basic_type(spec: &TypeSpec) -> Option<String> {
    Some(match spec {
        TypeSpec::Primitive(TypeName::I16 | TypeName::U16) => "INTEGER".to_owned(),
        TypeSpec::Primitive(TypeName::I32 | TypeName::U32) => "LONG".to_owned(),
        TypeSpec::Primitive(TypeName::F32) => "SINGLE".to_owned(),
        TypeSpec::Primitive(TypeName::F64) => "DOUBLE".to_owned(),
        TypeSpec::Named(name) => symbol(name),
        _ => return None,
    })
}

fn basic_field(spec: &TypeSpec) -> Option<String> {
    match width(spec) {
        Some(1) => Some("STRING * 1".to_owned()),
        _ if pointer(spec).is_some() => Some(if width(spec) == Some(4) { "LONG" } else { "INTEGER" }.to_owned()),
        _ => basic_type(spec),
    }
}

fn basic_parameter(name: &str, spec: &TypeSpec) -> Option<String> {
    // An adapter is BASIC's own by-reference argument.
    if let Some((_, adapter)) = Adapter::of(spec) {
        let element = || match spec {
            TypeSpec::Applied { args, .. } => match args.as_slice() {
                [TypeAnnotation::Value(element) | TypeAnnotation::Slice { element, .. }] => basic_type(element),
                _ => None,
            },
            _ => None,
        };
        return Some(match adapter {
            Adapter::Ref => format!("{name} AS {}", element()?),
            Adapter::String => format!("{name} AS STRING"),
            Adapter::Array => format!("{name}() AS {}", element()?),
        });
    }
    if let Some((far, _, target)) = pointer(spec) {
        return Some(if far {
            format!("SEG {name} AS {}", basic_type(target).unwrap_or_else(|| "ANY".to_owned()))
        } else {
            format!("BYVAL {name} AS INTEGER")
        });
    }
    // A byte is pushed as a word.
    let type_ = if width(spec) == Some(1) { "INTEGER".to_owned() } else { basic_type(spec)? };
    Some(format!("BYVAL {name} AS {type_}"))
}

/// The suffix of a BASIC function returning `spec`: a whole `ax` or `dx:ax`,
/// or under a BASIC ABI a float through its hidden result pointer.
fn basic_suffix(spec: &TypeSpec, abi: Abi) -> Option<&'static str> {
    match spec {
        TypeSpec::Primitive(TypeName::I16 | TypeName::U16) => Some("%"),
        TypeSpec::Primitive(TypeName::I32 | TypeName::U32) => Some("&"),
        TypeSpec::Primitive(TypeName::F32) if abi.basic().is_some() => Some("!"),
        TypeSpec::Primitive(TypeName::F64) if abi.basic().is_some() => Some("#"),
        TypeSpec::Primitive(TypeName::String) if abi.basic().is_some() => Some("$"),
        _ => None,
    }
}
