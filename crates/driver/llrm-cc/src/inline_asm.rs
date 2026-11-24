//! Clang's inline assembly (`call void asm sideeffect ...`) as llrm's
//! inline-code intrinsics.
//!
//! A run of adjacent asm calls is one block, assembled to bytes by the caller.
//! Each memory operand (`$n` in the text) becomes a placeholder displacement
//! that the intrinsic's name patches back: a local is a frame place of
//! `llrm.ia16.code.*`, which answers `dx:ax`; a global is a symbol fixup of
//! `llrm.ia16.asm.*`. A run that ends a function (the result is loaded from a
//! slot nothing stored) is the function's result, as Borland leaves it in
//! `dx:ax`.

/// A displacement no real instruction of the block carries; operand `k` takes
/// `PLACE + 4 * k`.
const PLACE: u32 = 0x7001;

#[derive(Debug, PartialEq)]
struct Call {
    text: String,
    constraints: String,
    /// Each operand's value: `@name` or `%name`, and its element type.
    operands: Vec<(String, String)>,
}

/// `text` with every asm run replaced; `assemble` turns Intel-syntax source
/// (16-bit code) into bytes.
pub fn translate(
    text: &str,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut declares: Vec<String> = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if is_empty_asm(lines[at]) {
            at += 1;
            continue;
        }
        if is_register_asm(lines[at]) {
            let (call, declare) = register_block(lines[at], assemble)?;
            out.push(call);
            // Equal blocks share a name, which is declared once.
            if !declares.contains(&declare) {
                declares.push(declare);
            }
            at += 1;
            continue;
        }
        if !is_asm(lines[at]) {
            out.push(lines[at].to_owned());
            at += 1;
            continue;
        }
        let mut run = Vec::new();
        while at < lines.len() && is_asm(lines[at]) && !is_empty_asm(lines[at]) {
            run.push(parse(lines[at])?);
            at += 1;
        }
        // The function's result: `%v = load iN, ptr %slot` then `ret iN %v`
        // right after the run.
        let result = lines.get(at).zip(lines.get(at + 1)).and_then(|(load, ret)| result_of(load, ret));
        if result.is_some() {
            at += 1;
        }
        let (call, declare) = block(&run, result.as_ref(), declares.len(), assemble)?;
        out.extend(call);
        declares.push(declare);
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    for declare in declares {
        joined.push_str(&declare);
        joined.push('\n');
    }
    Ok(joined)
}

// ---- Flat 32- and 64-bit code
// -------------------------------------------------------
//
// The same mechanism for flat code: an asm call whose operands sit at places
// the constraints name (`{eax}`, `{st}`, an address register for a memory
// operand) becomes a call of `llrm.ia32.asm.*` (`ia64` on x86-64), its bytes
// assembled here. A constraint that leaves the register to the allocator (`r`,
// `q`, `x`) takes the next register of the pool at the site, as `={..}` and
// `{..}` name one (DEVIATE: S2, the pin's `ISD::INLINEASM`, leaves the choice
// to the allocator); `g` and a constant are refused with their name.

/// The register file a flat block's mode names: the registers its target's
/// `@asm` line lists, with the family each belongs to.
struct Flat {
    /// 32 or 64: the mode, the width of an address register, the prefix of the
    /// block's name.
    bits: u8,
    asm: Vec<String>,
    registers: Vec<llrm_target::registers::Register>,
}

impl Flat {
    fn of(bits: u8) -> &'static Flat {
        static FLAT32: std::sync::LazyLock<Flat> =
            std::sync::LazyLock::new(|| Flat::parse(32, &llrm_target::Target::registers_text(&llrm_x86_m32::M32)));
        static FLAT64: std::sync::LazyLock<Flat> =
            std::sync::LazyLock::new(|| Flat::parse(64, &llrm_target::Target::registers_text(&llrm_x86_m64::M64)));
        if bits == 64 { &FLAT64 } else { &FLAT32 }
    }

    fn parse(
        bits: u8,
        text: &str,
    ) -> Flat {
        Flat {
            bits,
            asm: llrm_target::registers::asm_names(text),
            registers: llrm_target::registers::parse(text).expect("a target's register file parses"),
        }
    }

    /// A constraint's register as the block's name carries it: `st` is `st0`,
    /// `st(1)` is `st1`.
    fn register(
        &self,
        name: &str,
    ) -> Result<String, String> {
        let name = match name {
            "st" | "st(0)" => "st0",
            "st(1)" => "st1",
            other => other,
        };
        if self.asm.iter().any(|one| one == name) {
            Ok(name.to_owned())
        } else {
            Err(format!("asm: register {name} is not carried"))
        }
    }

    /// The register a name is a view of, to keep an address register off the
    /// ones the block names.
    fn root(
        &self,
        name: &str,
    ) -> String {
        self.registers.iter().find(|one| one.name == name).map_or(name, |one| one.root.as_str()).to_owned()
    }

    /// The view of `name`'s register an operand of the type `ty` takes in the
    /// block's text: `{cx}` holding an `i64` is `rcx`, an `i32` `ecx`. A
    /// register of no width (x87) and a type of none keep the name.
    fn view(
        &self,
        name: &str,
        ty: &str,
    ) -> String {
        let width = match ty.strip_prefix('i').and_then(|bits| bits.parse::<u32>().ok()) {
            Some(bits) => bits,
            None if ty == "ptr" => u32::from(self.bits),
            None => return name.to_owned(),
        };
        let root = self.root(name);
        self.registers
            .iter()
            .find(|one| one.root == root && one.bits == width && self.asm.contains(&one.name))
            .map_or(name, |one| one.name.as_str())
            .to_owned()
    }

    /// The vector registers an `x` operand may take, in the allocation order.
    fn vector_pool(&self) -> Vec<&str> {
        self.registers
            .iter()
            .filter(|one| one.name.starts_with("xmm") && self.asm.contains(&one.name))
            .map(|one| one.name.as_str())
            .collect()
    }

    /// The registers a pointer may be passed in: address width, in the
    /// allocation order.
    fn pool(&self) -> Vec<&str> {
        self.registers
            .iter()
            .filter(|one| u8::try_from(one.bits) == Ok(self.bits) && self.asm.contains(&one.name))
            .map(|one| one.name.as_str())
            .collect()
    }
}

/// What an operand is in a flat block's text.
enum Place {
    /// A register and the view of it the operand's type takes.
    Register(String, String),
    /// An address register the pointer argument is passed in.
    Memory(String),
}

/// `text` with every asm call of flat code replaced by a call of its
/// `llrm.ia32.asm.*` (`bits` 32) or `llrm.ia64.asm.*` (64) block.
pub fn translate_flat(
    text: &str,
    bits: u8,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    let flat = Flat::of(bits);
    let mut out: Vec<String> = Vec::new();
    let mut declares: Vec<String> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim_start();
        if !(trimmed.starts_with("call ") || trimmed.contains(" = call ")) || !trimmed.contains(" asm ") {
            out.push(line.to_owned());
            continue;
        }
        let (call, declare) = flat_block(line, flat, assemble)?;
        out.push(call);
        if !declares.contains(&declare) {
            declares.push(declare);
        }
    }
    let mut joined = out.join("\n");
    joined.push('\n');
    for declare in declares {
        joined.push_str(&declare);
        joined.push('\n');
    }
    Ok(joined)
}

/// The top-level `, `-separated items of `text` (not inside parentheses or
/// braces).
fn split_top(text: &str) -> Vec<&str> {
    let (mut depth, mut start, mut items) = (0_i32, 0, Vec::new());
    let bytes = text.as_bytes();
    for (at, &byte) in bytes.iter().enumerate() {
        match byte {
            b'(' | b'{' | b'[' => depth += 1,
            b')' | b'}' | b']' => depth -= 1,
            b',' if depth == 0 => {
                items.push(text[start..at].trim());
                start = at + 1;
            }
            _ => {}
        }
    }
    if !text[start..].trim().is_empty() {
        items.push(text[start..].trim());
    }
    items
}

fn flat_block(
    line: &str,
    flat: &Flat,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(String, String), String> {
    let line = line.trim();
    let (destination, call) = match line.split_once(" = call ") {
        Some((destination, rest)) => (Some(destination), rest),
        None => (None, line.strip_prefix("call ").ok_or("asm: not a call")?),
    };
    let (ret, rest) = call.split_once(" asm ").ok_or("asm: no asm")?;
    let ret = ret.trim_start_matches("addrspace(0) ").trim();
    let mut rest = rest;
    let (mut intel, mut volatile) = (false, false);
    loop {
        let Some(word) =
            ["sideeffect ", "inteldialect ", "alignstack ", "unwind "].iter().find(|w| rest.starts_with(**w))
        else {
            break;
        };
        intel |= *word == "inteldialect ";
        volatile |= *word == "sideeffect ";
        rest = &rest[word.len()..];
    }
    let (text, next) = quoted(rest, 0)?;
    let (constraints, next) = quoted(rest, next + 2)?;
    let arguments = rest[next..].strip_prefix('(').ok_or("asm: no arguments")?;
    let mut depth = 0;
    let end = arguments
        .char_indices()
        .find(|&(_, one)| {
            depth += i32::from(one == '(') - i32::from(one == ')');
            depth < 0
        })
        .map(|(at, _)| at)
        .ok_or("asm: unclosed arguments")?;
    // Each argument: its type and its value, without attributes.
    let arguments: Vec<(String, String)> = split_top(&arguments[..end])
        .into_iter()
        .map(|one| {
            let ty = one.split_whitespace().next().unwrap_or("").to_owned();
            let value = one.split_whitespace().last().unwrap_or("").to_owned();
            (ty, value)
        })
        .collect();
    let text = unescape(text)?;
    if text.contains('{') || text.contains("${") {
        return Err("asm: dialect alternatives and operand modifiers are not carried".into());
    }
    // Operands in LLVM's order: the constraints that are not clobbers.
    let mut operands: Vec<(String, Option<Place>)> = Vec::new();
    let (mut inputs, mut outputs, mut clobbers) = (Vec::<String>::new(), Vec::<String>::new(), Vec::<String>::new());
    let mut memory = volatile;
    let mut next_argument = 0;
    let mut input_values: Vec<(String, String)> = Vec::new();
    let mut named: Vec<String> = Vec::new();
    // First pass: the registers the block names, so an address register avoids
    // them.
    for item in constraints.split(',') {
        let inner = item.trim_start_matches(['=', '+', '&']);
        if let Some(name) = inner.strip_prefix('{').and_then(|one| one.strip_suffix('}')) {
            named.push(flat.root(&flat.register(name)?));
        } else if let Some(name) = item.strip_prefix("~{").and_then(|one| one.strip_suffix('}')) {
            if !["memory", "dirflag", "fpsr", "flags", "cc"].contains(&name) {
                named.push(flat.root(&flat.register(name)?));
            }
        }
    }
    let mut pool: Vec<&str> = flat.pool().into_iter().filter(|one| !named.contains(&flat.root(one))).collect();
    let mut direct_outputs: Vec<String> = Vec::new();
    // The types the outputs come back as, in order: the result, or its fields.
    let mut output_types = match ret {
        "void" => Vec::new(),
        aggregate if aggregate.starts_with('{') => split_top(aggregate.trim_start_matches('{').trim_end_matches('}'))
            .into_iter()
            .map(|one| one.trim().to_owned())
            .collect(),
        one => vec![one.to_owned()],
    }
    .into_iter();
    for item in constraints.split(',') {
        let item = register_alternative(item).unwrap_or_else(|| item.to_owned());
        let item = item.as_str();
        if let Some(name) = item.strip_prefix("~{").and_then(|one| one.strip_suffix('}')) {
            match name {
                "memory" => memory = true,
                "dirflag" | "fpsr" | "flags" | "cc" => {}
                other => clobbers.push(flat.register(other)?),
            }
            continue;
        }
        if item.contains('&') || item.starts_with('+') {
            return Err(format!("asm: constraint {item} is not carried"));
        }
        if item == "=*m" || item == "*m" {
            let argument = arguments.get(next_argument).ok_or("asm: an operand without an argument")?;
            next_argument += 1;
            let register = if pool.is_empty() {
                return Err("asm: no register left for a memory operand".into());
            } else {
                pool.remove(0).to_owned()
            };
            memory = true;
            operands.push((item.to_owned(), Some(Place::Memory(register.clone()))));
            inputs.push(register);
            input_values.push(argument.clone());
        } else if let Some(name) = item.strip_prefix("={").and_then(|one| one.strip_suffix('}')) {
            let register = flat.register(name)?;
            let view = flat.view(&register, &output_types.next().unwrap_or_default());
            operands.push((item.to_owned(), Some(Place::Register(register.clone(), view))));
            outputs.push(register.clone());
            direct_outputs.push(register);
        } else if let Some(name) = item.strip_prefix('{').and_then(|one| one.strip_suffix('}')) {
            let register = flat.register(name)?;
            let argument = arguments.get(next_argument).ok_or("asm: an operand without an argument")?;
            next_argument += 1;
            let view = flat.view(&register, &argument.0);
            operands.push((item.to_owned(), Some(Place::Register(register.clone(), view))));
            inputs.push(register);
            input_values.push(argument.clone());
        } else if matches!(item, "r" | "q" | "x" | "=r" | "=q" | "=x") {
            let output = item.starts_with('=');
            let register = if item.ends_with('x') {
                let mut vectors = flat.vector_pool();
                vectors.retain(|one| !named.contains(&flat.root(one)));
                vectors.retain(|one| {
                    !operands.iter().any(|(_, place)| matches!(place, Some(Place::Register(taken, _)) if taken == one))
                });
                vectors.first().map(|one| (*one).to_owned())
            } else if pool.is_empty() {
                None
            } else {
                Some(pool.remove(0).to_owned())
            }
            .ok_or("asm: no register left for an operand")?;
            if output {
                let view = flat.view(&register, &output_types.next().unwrap_or_default());
                operands.push((item.to_owned(), Some(Place::Register(register.clone(), view))));
                outputs.push(register.clone());
                direct_outputs.push(register);
            } else {
                let argument = arguments.get(next_argument).ok_or("asm: an operand without an argument")?;
                next_argument += 1;
                let view = flat.view(&register, &argument.0);
                operands.push((item.to_owned(), Some(Place::Register(register.clone(), view))));
                inputs.push(register);
                input_values.push(argument.clone());
            }
        } else if !item.is_empty() && item.bytes().all(|one| one.is_ascii_digit()) {
            let tied: usize = item.parse().map_err(|_| "asm: a tie")?;
            let register = match operands.get(tied) {
                Some((_, Some(Place::Register(register, _)))) if direct_outputs.contains(register) => register.clone(),
                _ => return Err(format!("asm: constraint {item} ties to an operand that is not a fixed output")),
            };
            let argument = arguments.get(next_argument).ok_or("asm: an operand without an argument")?;
            next_argument += 1;
            let view = flat.view(&register, &argument.0);
            operands.push((item.to_owned(), Some(Place::Register(register.clone(), view))));
            inputs.push(register);
            input_values.push(argument.clone());
        } else {
            return Err(format!("asm: constraint {item} is not carried"));
        }
    }
    if next_argument != arguments.iter().filter(|one| !one.0.is_empty()).count() {
        return Err("asm: the arguments do not match the constraints".into());
    }
    let mode = if flat.bits == 64 { ".code64\n" } else { ".code32\n" };
    let mut source = String::from(if intel { ".intel_syntax noprefix\n" } else { "" });
    source.push_str(mode);
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        if chars[at] == '$' && chars.get(at + 1) == Some(&'$') {
            source.push('$');
            at += 2;
        } else if chars[at] == '$' && chars.get(at + 1).is_some_and(char::is_ascii_digit) {
            let mut end = at + 1;
            while chars.get(end).is_some_and(char::is_ascii_digit) {
                end += 1;
            }
            let n: usize = chars[at + 1..end].iter().collect::<String>().parse().unwrap_or(usize::MAX);
            let place = operands
                .get(n)
                .and_then(|(_, place)| place.as_ref())
                .ok_or("asm: an operand number past the constraints")?;
            source.push_str(&match (place, intel) {
                (Place::Memory(register), false) => format!("(%{register})"),
                (Place::Memory(register), true) => format!("[{register}]"),
                (Place::Register(_, view), false) => format!("%{view}"),
                (Place::Register(_, view), true) => view.clone(),
            });
            at = end;
        } else {
            source.push(chars[at]);
            at += 1;
        }
    }
    source.push('\n');
    // An x87 input in st(1) that the block neither clobbers nor returns stays
    // on the stack after it (`fscale` leaves it): `fstp st(1)` pops it, as
    // `reg-stack` pops a dead stack input.
    if inputs.iter().any(|one| one == "st1") && !clobbers.iter().chain(&outputs).any(|one| one == "st1") {
        source.push_str(if intel { "fstp st(1)\n" } else { "fstp %st(1)\n" });
    }
    let bytes = assemble(&source)?;
    let name = llrm_mir::intrinsics::asm_name(&llrm_mir::intrinsics::AsmBlock {
        bits: flat.bits,
        code: bytes,
        inputs,
        outputs,
        clobbers,
        memory,
        fixups: Vec::new(),
    });
    let types = input_values.iter().map(|(ty, _)| ty.as_str()).collect::<Vec<_>>().join(", ");
    let values = input_values.iter().map(|(ty, value)| format!("{ty} {value}")).collect::<Vec<_>>().join(", ");
    let call =
        format!("  {}call {ret} @\"{name}\"({values})", destination.map_or(String::new(), |one| format!("{one} = ")));
    Ok((call, format!("declare {ret} @\"{name}\"({types})")))
}

/// An asm call that names registers: `%r = call { i16, i16 } asm sideeffect
/// "..", "={dx},={ax},{ax}"(i16 %x)`, which `#pragma aux` code comes out as.
fn is_register_asm(line: &str) -> bool {
    let line = line.trim_start();
    if !(line.contains("call ") && line.contains(" asm sideeffect ")) {
        return false;
    }
    let Some(from) = line.find(" asm sideeffect ") else { return false };
    let mut at = from + " asm sideeffect ".len();
    for word in ["inteldialect ", "alignstack ", "unwind "] {
        if line[at..].starts_with(word) {
            at += word.len();
        }
    }
    let Ok((_, next)) = quoted(line, at) else { return false };
    let Ok((constraints, _)) = quoted(line, next + 2) else { return false };
    constraints.split(',').any(|one| one.starts_with("={") || one.starts_with('{'))
}

/// The registers inline asm may name, from the 16-bit target's register file
/// (`@asm`): the one description that says, not a list here.
fn asm_registers() -> &'static [String] {
    static NAMES: std::sync::LazyLock<Vec<String>> = std::sync::LazyLock::new(|| {
        llrm_target::registers::asm_names(&llrm_target::Target::registers_text(&llrm_x86_m16::M16))
    });
    &NAMES
}

/// A register's name as the backend takes it: its width is the name's.
fn register_name(name: &str) -> Result<&str, String> {
    if asm_registers().iter().any(|one| one == name) {
        Ok(name)
    } else {
        Err(format!("asm: register {name} is not carried"))
    }
}

/// One register-declared asm call as a call of `llrm.ia16.asm.*`, and the
/// declaration of that.
fn register_block(
    line: &str,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(String, String), String> {
    let line = line.trim();
    let (destination, call) = match line.split_once(" = call ") {
        Some((destination, rest)) => (Some(destination), rest),
        None => (None, line.strip_prefix("call ").ok_or("asm: not a call")?),
    };
    let (ret, rest) = call.split_once(" asm sideeffect ").ok_or("asm: no asm")?;
    let ret = ret.trim_start_matches("addrspace(0) ").trim();
    let mut at = 0;
    for word in ["inteldialect ", "alignstack ", "unwind "] {
        if rest[at..].starts_with(word) {
            at += word.len();
        }
    }
    let (text, next) = quoted(rest, at)?;
    let (constraints, next) = quoted(rest, next + 2)?;
    let arguments = rest[next..].strip_prefix('(').ok_or("asm: no arguments")?;
    let mut depth = 0;
    let end = arguments
        .char_indices()
        .find(|&(_, one)| {
            depth += i32::from(one == '(') - i32::from(one == ')');
            depth < 0
        })
        .map(|(at, _)| at)
        .ok_or("asm: unclosed arguments")?;
    let arguments = &arguments[..end];
    let (mut inputs, mut outputs, mut clobbers, mut memory) = (Vec::new(), Vec::new(), Vec::new(), false);
    for item in constraints.split(',') {
        if let Some(name) = item.strip_prefix("={").and_then(|one| one.strip_suffix('}')) {
            outputs.push(register_name(name)?.to_owned());
        } else if let Some(name) = item.strip_prefix('{').and_then(|one| one.strip_suffix('}')) {
            inputs.push(register_name(name)?.to_owned());
        } else if let Some(name) = item.strip_prefix("~{").and_then(|one| one.strip_suffix('}')) {
            if name == "memory" {
                memory = true;
            } else if !["dirflag", "fpsr", "flags"].contains(&name) {
                clobbers.push(register_name(name)?.to_owned());
            }
        } else {
            return Err(format!("asm: constraint {item} is not carried"));
        }
    }
    let bytes = assemble(&format!(".intel_syntax noprefix\n.code16\n{}\n", unescape(text)?))?;
    let name = llrm_mir::intrinsics::asm_name(&llrm_mir::intrinsics::AsmBlock {
        bits: 16,
        code: bytes,
        inputs,
        outputs,
        clobbers,
        memory,
        fixups: Vec::new(),
    });
    let types = arguments
        .split(", ")
        .filter(|one| !one.is_empty())
        .map(|one| one.split_once(' ').map_or(one, |(ty, _)| ty).to_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let call = format!(
        "  {}call {ret} @\"{name}\"({arguments})",
        destination.map_or(String::new(), |one| format!("{one} = "))
    );
    Ok((call, format!("declare {ret} @\"{name}\"({types})")))
}

/// `asm volatile ("" : : "g" (p) : "memory")`: no instruction, only an
/// optimisation barrier. It is dropped: the operands need not be memory places,
/// and what it would have kept alive is the optimiser's to fold (the program's
/// result does not depend on it).
fn is_empty_asm(line: &str) -> bool {
    let Some(at) = line.find(" asm sideeffect ") else { return false };
    if !is_asm(line) || line.contains(" = call ") {
        return false;
    }
    let mut rest = &line[at + " asm sideeffect ".len()..];
    for word in ["inteldialect ", "alignstack ", "unwind "] {
        rest = rest.strip_prefix(word).unwrap_or(rest);
    }
    rest.starts_with("\"\",")
}

fn is_asm(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("call ") && line.contains(" asm sideeffect ")
}

/// `(destination, type)` of a function's result loaded from a slot and
/// returned.
fn result_of(
    load: &str,
    ret: &str,
) -> Option<(String, String)> {
    let (destination, rest) = load.trim().split_once(" = load ")?;
    let (ty, _) = rest.split_once(',')?;
    (ret.trim() == format!("ret {ty} {destination}")).then(|| (destination.to_owned(), ty.to_owned()))
}

fn unescape(quoted: &str) -> Result<String, String> {
    let bytes = quoted.as_bytes();
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] == b'\\' {
            let hex = quoted.get(at + 1..at + 3).ok_or("asm: a bad escape")?;
            if hex == "\\\\" || bytes.get(at + 1) == Some(&b'\\') {
                out.push(b'\\');
                at += 2;
            } else {
                out.push(u8::from_str_radix(hex, 16).map_err(|_| format!("asm: escape \\{hex}"))?);
                at += 3;
            }
        } else {
            out.push(bytes[at]);
            at += 1;
        }
    }
    String::from_utf8(out).map_err(|_| "asm: text is not UTF-8".to_owned())
}

/// The text between the quote that opens at `from` and the one that closes it.
fn quoted(
    line: &str,
    from: usize,
) -> Result<(&str, usize), String> {
    let bytes = line.as_bytes();
    if bytes.get(from) != Some(&b'"') {
        return Err(format!("asm: expected a string in `{line}`"));
    }
    let mut at = from + 1;
    while at < bytes.len() && bytes[at] != b'"' {
        at += if bytes[at] == b'\\' { 2 } else { 1 };
    }
    Ok((&line[from + 1..at], at + 1))
}

fn parse(line: &str) -> Result<Call, String> {
    if line.contains(" = call ") {
        return Err("asm: a block that answers a value is not carried".into());
    }
    let after = line.find(" asm sideeffect ").ok_or("asm: no asm")? + " asm sideeffect ".len();
    let mut at = after;
    for word in ["inteldialect ", "alignstack ", "unwind "] {
        if line[at..].starts_with(word) {
            at += word.len();
        }
    }
    let (text, next) = quoted(line, at)?;
    let (constraints, next) = quoted(line, next + 2)?;
    let rest = line[next..].strip_prefix('(').ok_or("asm: no arguments")?;
    let mut depth = 0;
    let end = rest
        .char_indices()
        .find(|&(_, one)| {
            depth += i32::from(one == '(') - i32::from(one == ')');
            depth < 0
        })
        .map(|(at, _)| at)
        .ok_or("asm: unclosed arguments")?;
    let args = &rest[..end];
    let mut operands = Vec::new();
    for argument in args.split(", ptr ").filter(|one| !one.trim().is_empty()) {
        let argument = argument.trim().trim_start_matches("ptr ");
        let rest = argument.strip_prefix("elementtype(").ok_or("asm: an operand that is not a memory place")?;
        let (element, value) = rest.split_once(") ").ok_or("asm: an operand")?;
        operands.push((value.trim().to_owned(), element.to_owned()));
    }
    Ok(Call { text: unescape(text)?, constraints: constraints.to_owned(), operands })
}

/// The call lines replacing a run, and the declaration of what they call.
fn block(
    run: &[Call],
    result: Option<&(String, String)>,
    number: usize,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<(Vec<String>, String), String> {
    let mut operands: Vec<&(String, String)> = Vec::new();
    let mut source = String::from(".intel_syntax noprefix\n.code16\n");
    for call in run {
        let base = operands.len();
        operands.extend(call.operands.iter());
        source.push_str(&renumber(&call.text, base, &operands, &call.operands));
        source.push('\n');
    }
    let globals = operands.iter().filter(|(value, _)| value.starts_with('@')).count();
    if globals != 0 && globals != operands.len() {
        return Err("asm: a block that names both locals and globals is not carried".into());
    }
    let bytes = assemble(&source)?;
    let mut places = Vec::new();
    for (k, _) in operands.iter().enumerate() {
        let want = (PLACE + 4 * k as u32) as u16;
        let found: Vec<usize> = (0..bytes.len().saturating_sub(1))
            .filter(|&at| u16::from_le_bytes([bytes[at], bytes[at + 1]]) == want)
            .collect();
        match found[..] {
            [at] => places.push(at),
            _ => return Err(format!("asm: operand {k} appears {} times in the assembled bytes", found.len())),
        }
    }
    let hex: String = bytes.iter().map(|one| format!("{one:02x}")).collect();
    let pointers = operands.iter().map(|(value, _)| format!("ptr {value}")).collect::<Vec<_>>().join(", ");
    let types = operands.iter().map(|_| "ptr").collect::<Vec<_>>().join(", ");
    if globals != 0 {
        let fixups: String = places.iter().map(|at| format!(".f{at}_offset_0")).collect();
        let name = format!("llrm.ia16.asm.{hex}.-.-.-.m{fixups}");
        return Ok((
            vec![format!("  call void @\"{name}\"({pointers})")],
            format!("declare void @\"{name}\"({types})"),
        ));
    }
    let placed: String = places.iter().map(|at| format!(".{at}p0")).collect();
    let name = format!("llrm.ia16.code.{hex}{placed}");
    let declare = format!("declare i32 @\"{name}\"({types})");
    let call = |destination: &str| format!("  {destination}call i32 @\"{name}\"({pointers})");
    let unused = format!("%asm.{number}");
    let lines = match result {
        Some((value, ty)) if ty == "i32" => vec![call(&format!("{value} = "))],
        Some((value, ty)) if ty == "i16" => {
            vec![call(&format!("{unused} = ")), format!("  {value} = trunc i32 {unused} to i16")]
        }
        Some((_, ty)) => return Err(format!("asm: a result of type {ty} is not carried")),
        None => vec![call(&format!("{unused} = "))],
    };
    Ok((lines, declare))
}

/// `text` with `$n` (n among this call's operands) replaced by the placeholder
/// of operand `base + n`.
fn renumber(
    text: &str,
    base: usize,
    all: &[&(String, String)],
    own: &[(String, String)],
) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut at = 0;
    while at < chars.len() {
        if chars[at] == '$' && chars.get(at + 1) == Some(&'$') {
            at += 2;
        } else if chars[at] == '$' && chars.get(at + 1).is_some_and(char::is_ascii_digit) {
            let mut end = at + 1;
            while chars.get(end).is_some_and(char::is_ascii_digit) {
                end += 1;
            }
            let n: usize = chars[at + 1..end].iter().collect::<String>().parse().unwrap_or(0);
            let k = base + n;
            let displacement = PLACE + 4 * k as u32;
            let _ = (all, own);
            out.push_str(&if all[k].0.starts_with('@') {
                format!("[{displacement:#x}]")
            } else {
                format!("[bp+{displacement:#x}]")
            });
            at = end;
        } else {
            out.push(chars[at]);
            at += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The registers an asm constraint may name come from the target's `@asm`
    /// line: the whole-value ones, not the frame, the stack, a high byte or a
    /// segment register.
    #[test]
    fn a_register_constraint_names_what_the_target_description_lists() {
        for name in ["ax", "dl", "esi", "edi", "bx"] {
            assert_eq!(register_name(name), Ok(name));
        }
        for name in ["bp", "sp", "ah", "ds", "xmm0", ""] {
            assert!(register_name(name).unwrap_err().contains("not carried"), "{name}");
        }
        assert_eq!(asm_registers().len(), 16);
    }

    fn fake(source: &str) -> Result<Vec<u8>, String> {
        assert!(source.contains("[bp+0x7001]") || source.contains("[0x7001]"), "{source}");
        Ok(vec![0x89, 0xa6, 0x01, 0x70, 0xc3])
    }

    #[test]
    fn an_empty_asm_with_non_memory_operands_is_dropped() {
        // pr56866: `asm volatile ("" : : "g" (array))` refused with "an operand
        // that is not a memory place"
        let ir = "define i32 @f() {\n  %1 = alloca [4 x i32]\n  call void asm sideeffect \"\", \"imr,~{memory}\"(ptr %1) #1, !srcloc !6\n  ret i32 0\n}\n";
        let out = translate(ir, &mut fake).unwrap();
        assert!(!out.contains("asm") && out.contains("ret i32 0"), "{out}");
    }

    #[test]
    fn a_local_operand_is_a_frame_place_of_inline_code() {
        let ir = "define i32 @f() {\n  %1 = alloca i16\n  call void asm sideeffect inteldialect \"mov word ptr $0, sp\", \"=*m\"(ptr elementtype(i16) %1)\n  %2 = load i32, ptr %9, align 2\n  ret i32 %2\n}\n";
        let out = translate(ir, &mut fake).unwrap();
        assert!(out.contains("%2 = call i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr %1)"), "{out}");
        assert!(
            out.contains("ret i32 %2")
                && out.contains("declare i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr)")
                && !out.contains("load"),
            "{out}"
        );
    }

    #[test]
    fn a_global_operand_is_a_symbol_fixup_of_inline_asm() {
        let ir = "define void @g() {\n  call void asm sideeffect inteldialect \"mov $0, sp\", \"=*m\"(ptr elementtype(i16) @sp_now) #1, !srcloc !3\n  ret void\n}\n";
        let out = translate(ir, &mut fake).unwrap();
        assert!(out.contains("call void @\"llrm.ia16.asm.89a60170c3.-.-.-.m.f2_offset_0\"(ptr @sp_now)"), "{out}");
    }

    #[test]
    fn adjacent_calls_are_one_block_with_operands_numbered_across_them() {
        let ir = "  call void asm sideeffect \"push ebx\", \"~{esp}\"()\n  call void asm sideeffect inteldialect \"mov $0, ax\", \"=*m\"(ptr elementtype(i16) %7)\n";
        let mut seen = String::new();
        let mut record = |source: &str| {
            seen = source.to_owned();
            fake(source)
        };
        translate(ir, &mut record).unwrap();
        assert!(seen.contains("push ebx\n") && seen.contains("mov [bp+0x7001], ax"), "{seen}");
    }
}

#[cfg(test)]
mod register_tests {
    use super::*;

    /// `#pragma aux split = "cwd" "idiv cx" parm [ax] [cx] value [dx ax]` came
    /// out of clang as an asm call with registers
    /// and was dropped ("a block that answers a value is not carried"); it is
    /// now the register-declared block.
    #[test]
    fn a_call_with_registers_is_a_register_declared_block() {
        let ir = "  %2 = call addrspace(0) { i16, i16, i16 } asm sideeffect inteldialect \"cwd\\0Aidiv cx\\0A\", \"={dx},={ax},={cx},{ax},{cx},~{memory}\"(i16 %0, i16 %1)\n";
        let out = translate(ir, &mut |source: &str| {
            assert!(source.contains("cwd\nidiv cx"), "{source}");
            Ok(vec![0x99, 0xf7, 0xf9])
        })
        .unwrap();
        let name = "llrm.ia16.asm.99f7f9.ax_cx.dx_ax_cx.-.m";
        assert!(out.contains(&format!("%2 = call {{ i16, i16, i16 }} @\"{name}\"(i16 %0, i16 %1)")), "{out}");
        assert!(out.contains(&format!("declare {{ i16, i16, i16 }} @\"{name}\"(i16, i16)")), "{out}");
        assert!(llrm_mir::intrinsics::asm(name).is_some());
    }

    #[test]
    fn a_byte_register_keeps_its_width_in_the_name() {
        let ir = "  %2 = call addrspace(0) i8 asm sideeffect inteldialect \"add al,bl\\0A\", \"={al},{al},{bl},~{memory}\"(i8 %0, i8 %1)\n";
        let out = translate(ir, &mut |_| Ok(vec![0x00, 0xd8])).unwrap();
        assert!(out.contains("llrm.ia16.asm.00d8.al_bl.al.-.m"), "{out}");
    }
}

/// A multi-alternative constraint (`rm`, `g`, `imr`) reduced to the one
/// alternative the block takes (`ComputeConstraintToUse`): a direct operand
/// with a register alternative is a register, as LLVM prefers the register
/// class; an indirect one (`=*imr`, clang's form of an output it keeps in
/// memory) is the memory operand, its address in a register.
fn register_alternative(item: &str) -> Option<String> {
    let letters = item.trim_start_matches(['=', '*']);
    let prefix = &item[..item.len() - letters.len()];
    let alternatives = letters.len() > 1 || letters == "g";
    if !alternatives || !letters.bytes().all(|one| b"rmgiqn".contains(&one)) {
        return None;
    }
    if prefix.contains('*') {
        return letters.bytes().any(|one| matches!(one, b'm' | b'g')).then(|| format!("{prefix}m"));
    }
    letters.bytes().any(|one| matches!(one, b'r' | b'g' | b'q')).then(|| format!("{prefix}r"))
}
