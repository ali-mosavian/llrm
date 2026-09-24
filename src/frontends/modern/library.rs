//! The number types' library methods (section 3): checked and saturating
//! arithmetic and checked conversion, written in the language. Each is
//! compiled only where called.

use super::desugar::desugar;
use super::error::Diagnostic;
use super::lexer::lex;
use super::parser::parse;
use super::syntax::{Expr, Function, Module, ParameterType, TypeAnnotation, TypeSpec};

/// Each integer type: its name, whether it is signed, and its bounds.
const INTEGERS: [(&str, bool, i64, i64); 6] = [
    ("i8", true, -128, 127),
    ("u8", false, 0, 255),
    ("i16", true, -32768, 32767),
    ("u16", false, 0, 65535),
    ("i32", true, -2147483648, 2147483647),
    ("u32", false, 0, 4294967295),
];

/// An operation: its name, its operator, and, over `self`, `other` and the
/// wrapped `result`, when it overflowed and the bound it then saturates to,
/// signed and unsigned.
struct Operation {
    name: &'static str,
    operator: &'static str,
    signed: (&'static str, &'static str),
    unsigned: (&'static str, &'static str),
}

const OPERATIONS: [Operation; 3] = [
    Operation {
        name: "add",
        operator: "+",
        signed: ("((self ^ result) & (other ^ result)) < 0", "other < 0 ? MIN : MAX"),
        unsigned: ("result < self", "MAX"),
    },
    Operation {
        name: "sub",
        operator: "-",
        signed: ("((self ^ other) & (self ^ result)) < 0", "other < 0 ? MAX : MIN"),
        unsigned: ("self < other", "MIN"),
    },
    Operation {
        name: "mul",
        operator: "*",
        // The quotient check divides only where it cannot fault.
        signed: (
            "self == -1 ? other == MIN : (self != 0 && result // self != other)",
            "(self < 0) != (other < 0) ? MIN : MAX",
        ),
        unsigned: ("self != 0 && result // self != other", "MAX"),
    },
];

/// `Hashable` for `module`'s own structs and payload-free enums, as their
/// fields' methods combine; a type that defines its own keeps it.
pub fn derived(module: &Module) -> Result<Vec<Function>, Diagnostic> {
    let defines = |type_: &str, method: &str| module.functions.iter().any(|one| one.name == format!("{type_}.{method}"));
    let mut functions = Vec::new();
    for one in module.structs.iter().filter(|one| one.generics.is_empty() && one.bits.is_none()) {
        let mut out = String::new();
        if !defines(&one.name, "hash") {
            out.push_str("fn SELF.hash(self: &SELF) -> u16:\n    let mut hash: u16 = 0\n");
            for field in &one.fields {
                out.push_str(&format!("    hash = hash * 31 + self.{}.hash()\n", field.name));
            }
            out.push_str("    return hash\n\n");
        }
        if !defines(&one.name, "eq") {
            let fields: Vec<String> = one.fields.iter().map(|field| format!("self.{0}.eq(other.{0})", field.name)).collect();
            let all = if fields.is_empty() { "true".to_owned() } else { fields.join(" && ") };
            out.push_str(&format!("fn SELF.eq(self: &SELF, other: &SELF) -> bool:\n    return {all}\n\n"));
        }
        functions.extend(for_type(&out, &one.name)?);
    }
    for one in module.enums.iter().filter(|one| one.generics.is_empty() && one.variants.iter().all(|variant| variant.fields.is_empty())) {
        let mut out = String::new();
        if !defines(&one.name, "hash") {
            out.push_str("fn SELF.hash(self: SELF) -> u16:\n    return u16(self)\n\n");
        }
        if !defines(&one.name, "eq") {
            out.push_str("fn SELF.eq(self: SELF, other: SELF) -> bool:\n    return self == other\n\n");
        }
        functions.extend(for_type(&out, &one.name)?);
    }
    Ok(functions)
}

/// `source`'s methods of `SELF`, made methods of `type_`: a module's type
/// is named with dots the parser would not take.
fn for_type(source: &str, type_: &str) -> Result<Vec<Function>, Diagnostic> {
    let mut module = parse(lex(source)?)?;
    desugar(&mut module)?;
    let renamed = |annotation: &mut TypeAnnotation| {
        if let TypeAnnotation::Value(TypeSpec::Named(name)) = annotation {
            *name = type_.to_owned();
        }
    };
    for function in &mut module.functions {
        function.name = function.name.replacen("SELF", type_, 1);
        for parameter in &mut function.parameters {
            match &mut parameter.type_ {
                ParameterType::Owned(annotation) | ParameterType::Borrowed { target: annotation, .. } => renamed(annotation),
            }
        }
    }
    Ok(module.functions)
}

/// The library's methods.
pub fn functions() -> Result<Vec<Function>, Diagnostic> {
    let mut library = parse(lex(&source())?)?;
    desugar(&mut library)?;
    for function in &mut library.functions {
        // A header cannot spell `checked_to[i8]`.
        if let Some((method, target)) = function.name.split_once(".checked_to_") {
            function.name = format!("{method}.checked_to[{target}]");
        }
    }
    Ok(library.functions)
}

fn source() -> String {
    let mut out = String::new();
    for (type_, signed, min, max) in INTEGERS {
        for operation in &OPERATIONS {
            let (overflowed, saturated) = if signed { operation.signed } else { operation.unsigned };
            let bounded = |text: &str| text.replace("MIN", &min.to_string()).replace("MAX", &max.to_string());
            let (name, operator) = (operation.name, operation.operator);
            let (overflowed, saturated) = (bounded(overflowed), bounded(saturated));
            out.push_str(&format!(
                "fn {type_}.checked_{name}(self: {type_}, other: {type_}) -> Option[{type_}]:\n\
                 \x20   let result = {type_}(self {operator} other)\n\
                 \x20   if {overflowed}:\n\
                 \x20       return .none\n\
                 \x20   return .some(result)\n\n\
                 fn {type_}.saturating_{name}(self: {type_}, other: {type_}) -> {type_}:\n\
                 \x20   let result = {type_}(self {operator} other)\n\
                 \x20   if {overflowed}:\n\
                 \x20       return {saturated}\n\
                 \x20   return result\n\n"
            ));
        }
    }
    let floats = [("f32", f64::from(f32::MIN), f64::from(f32::MAX)), ("f64", f64::MIN, f64::MAX)];
    let sources = INTEGERS.iter().map(|&(name, _, min, max)| (name, min as f64, max as f64)).chain(floats);
    for (type_, low, high) in sources {
        for (target, _, min, max) in INTEGERS {
            // A float in range truncates into it; NaN compares false.
            let mut kept = Vec::new();
            if (min as f64) > low {
                kept.push(format!("self >= {min}{}", point(type_)));
            }
            if (max as f64) < high {
                kept.push(if type_.starts_with('f') { format!("self < {}.0", max + 1) } else { format!("self <= {max}") });
            }
            let kept = if kept.is_empty() { "true".to_owned() } else { kept.join(" && ") };
            out.push_str(&format!(
                "fn {type_}.checked_to_{target}(self: {type_}) -> Option[{target}]:\n\
                 \x20   if !({kept}):\n\
                 \x20       return .none\n\
                 \x20   return .some({target}(self))\n\n"
            ));
        }
    }
    out.push_str(&protocols());
    out
}

/// `Hashable` and `Ordered` (section 3) for the scalar types and `string`.
fn protocols() -> String {
    let mut out = String::new();
    let wide = |type_: &str| type_.ends_with("32");
    let scalars = INTEGERS.iter().map(|one| one.0).chain(["char", "bool", "f32", "f64"]);
    for type_ in scalars {
        if !type_.starts_with('f') {
            // A 32-bit value folds its high word into the low one.
            let hashed = if wide(type_) { "u16(self ^ (self >> 16))" } else { "u16(self)" };
            out.push_str(&format!("fn {type_}.hash(self: {type_}) -> u16:\n    return {hashed}\n\n"));
        }
        out.push_str(&format!(
            "fn {type_}.eq(self: {type_}, other: {type_}) -> bool:\n    return self == other\n\n"
        ));
        if type_ != "bool" {
            out.push_str(&format!(
                "fn {type_}.cmp(self: {type_}, other: {type_}) -> i8:\n    return self < other ? -1 : self > other ? 1 : 0\n\n"
            ));
        }
    }
    out.push_str(
        "fn string.hash(self: &string) -> u16:\n\
         \x20   let mut hash: u16 = 5381\n\
         \x20   for c in self:\n\
         \x20       hash = hash * 33 ^ u16(c)\n\
         \x20   return hash\n\n\
         fn string.eq(self: &string, other: &string) -> bool:\n    return self == other\n\n\
         fn string.cmp(self: &string, other: &string) -> i8:\n    return self < other ? -1 : self > other ? 1 : 0\n\n",
    );
    out
}

/// What makes an integer literal a float of `type_`.
fn point(type_: &str) -> &'static str {
    if type_.starts_with('f') { ".0" } else { "" }
}

/// A `main` returning `Result[void, E]` runs inside a generated `main`
/// returning the exit code: 0 for `.ok()`, 1 for `.err`.
pub fn entry(module: &mut Module) -> Result<(), Diagnostic> {
    let Some(main) = module.functions.iter_mut().find(|one| one.name == "main") else {
        return Ok(());
    };
    if !matches!(&main.result, TypeAnnotation::Value(TypeSpec::Applied { name, .. }) if name == "Result") {
        return Ok(());
    }
    main.name = "$main".to_owned();
    let source = "fn main() -> i16:\n    match ENTRY():\n        .ok(_):\n            return 0\n        .err(_):\n            return 1\n";
    let mut wrapper = parse(lex(source)?)?.functions.remove(0);
    for statement in &mut wrapper.body {
        let Ok(()) = statement.walk_mut(&mut |expression| -> Result<(), std::convert::Infallible> {
            if let Expr::Call { name, .. } = expression {
                *name = "$main".to_owned();
            }
            Ok(())
        });
    }
    module.functions.push(wrapper);
    Ok(())
}
