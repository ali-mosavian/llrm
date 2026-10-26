//! Clang's inline assembly (`call void asm sideeffect ...`) as llrm's inline-code intrinsics.
//!
//! A run of adjacent asm calls is one block, assembled to bytes by the caller. Each memory operand (`$n` in the text)
//! becomes a placeholder displacement that the intrinsic's name patches back: a local is a frame place of
//! `llrm.ia16.code.*`, which answers `dx:ax`; a global is a symbol fixup of `llrm.ia16.asm.*`. A run that ends a function
//! (the result is loaded from a slot nothing stored) is the function's result, as Borland leaves it in `dx:ax`.

/// A displacement no real instruction of the block carries; operand `k` takes `PLACE + 4 * k`.
const PLACE: u32 = 0x7001;

#[derive(Debug, PartialEq)]
struct Call {
    text: String,
    constraints: String,
    /// Each operand's value: `@name` or `%name`, and its element type.
    operands: Vec<(String, String)>,
}

/// `text` with every asm run replaced; `assemble` turns Intel-syntax source (16-bit code) into bytes.
pub fn translate(
    text: &str,
    assemble: &mut dyn FnMut(&str) -> Result<Vec<u8>, String>,
) -> Result<String, String> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<String> = Vec::new();
    let mut declares: Vec<String> = Vec::new();
    let mut at = 0;
    while at < lines.len() {
        if lines[at].starts_with("declare ") && lines[at].contains("@__emit__(") {
            at += 1;
            continue;
        }
        if !is_asm(lines[at]) {
            out.push(lines[at].to_owned());
            at += 1;
            continue;
        }
        let mut run = Vec::new();
        // `__emit__` answers an unused number; the replacing call keeps it so the numbering stays dense.
        let mut numbered = None;
        while at < lines.len() && is_asm(lines[at]) {
            if is_emit(lines[at]) && numbered.is_none() {
                numbered = lines[at].trim().split_once(" = ").map(|(name, _)| name.to_owned());
            }
            run.push(parse(lines[at])?);
            at += 1;
        }
        // The function's result: `%v = load iN, ptr %slot` then `ret iN %v` right after the run.
        let result = lines.get(at).zip(lines.get(at + 1)).and_then(|(load, ret)| result_of(load, ret));
        if result.is_some() {
            at += 1;
        }
        let (call, declare) = block(&run, result.as_ref(), numbered, declares.len(), assemble)?;
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

fn is_asm(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("call ") && line.contains(" asm sideeffect ") || is_emit(line) && (line.starts_with("call ") || line.contains(" = call "))
}

/// Borland's `__emit__(bytes...)`, a call to the library's declaration rather than an asm statement.
fn is_emit(line: &str) -> bool {
    line.contains("@__emit__(")
}

/// The bytes an `__emit__` call lists, as `.byte` text.
fn emit_bytes(line: &str) -> Result<String, String> {
    let args = line.split_once("@__emit__(").and_then(|(_, rest)| rest.split_once(')')).ok_or("asm: __emit__")?.0;
    let bytes: Result<Vec<String>, String> = args
        .split(',')
        .map(|one| {
            let value = one.split_whitespace().last().ok_or("asm: __emit__ argument")?;
            value.parse::<i64>().map(|n| (n & 0xff).to_string()).map_err(|_| "asm: __emit__ needs constant bytes".to_owned())
        })
        .collect();
    Ok(format!(".byte {}", bytes?.join(", ")))
}

/// `(destination, type)` of a function's result loaded from a slot and returned.
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
    if is_emit(line) {
        return Ok(Call { text: emit_bytes(line)?, constraints: String::new(), operands: Vec::new() });
    }
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
    numbered: Option<String>,
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
        let found: Vec<usize> =
            (0..bytes.len().saturating_sub(1)).filter(|&at| u16::from_le_bytes([bytes[at], bytes[at + 1]]) == want).collect();
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
        return Ok((vec![format!("  call void @\"{name}\"({pointers})")], format!("declare void @\"{name}\"({types})")));
    }
    let placed: String = places.iter().map(|at| format!(".{at}p0")).collect();
    let name = format!("llrm.ia16.code.{hex}{placed}");
    let declare = format!("declare i32 @\"{name}\"({types})");
    let call = |destination: &str| format!("  {destination}call i32 @\"{name}\"({pointers})");
    let unused = numbered.unwrap_or_else(|| format!("%asm.{number}"));
    let lines = match result {
        Some((value, ty)) if ty == "i32" && unused.starts_with("%asm.") => vec![call(&format!("{value} = "))],
        Some((value, ty)) if ty == "i32" => vec![call(&format!("{unused} = ")), format!("  {value} = add i32 {unused}, 0")],
        Some((value, ty)) if ty == "i16" => vec![call(&format!("{unused} = ")), format!("  {value} = trunc i32 {unused} to i16")],
        Some((_, ty)) => return Err(format!("asm: a result of type {ty} is not carried")),
        None => vec![call(&format!("{unused} = "))],
    };
    Ok((lines, declare))
}

/// `text` with `$n` (n among this call's operands) replaced by the placeholder of operand `base + n`.
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
            out.push_str(&if all[k].0.starts_with('@') { format!("[{displacement:#x}]") } else { format!("[bp+{displacement:#x}]") });
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

    fn fake(source: &str) -> Result<Vec<u8>, String> {
        assert!(source.contains("[bp+0x7001]") || source.contains("[0x7001]"), "{source}");
        Ok(vec![0x89, 0xa6, 0x01, 0x70, 0xc3])
    }

    #[test]
    fn a_local_operand_is_a_frame_place_of_inline_code() {
        let ir = "define i32 @f() {\n  %1 = alloca i16\n  call void asm sideeffect inteldialect \"mov word ptr $0, sp\", \"=*m\"(ptr elementtype(i16) %1)\n  %2 = load i32, ptr %9, align 2\n  ret i32 %2\n}\n";
        let out = translate(ir, &mut fake).unwrap();
        assert!(out.contains("%2 = call i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr %1)"), "{out}");
        assert!(out.contains("ret i32 %2") && out.contains("declare i32 @\"llrm.ia16.code.89a60170c3.2p0\"(ptr)") && !out.contains("load"), "{out}");
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
