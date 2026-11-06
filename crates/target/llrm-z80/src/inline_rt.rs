//! A runtime routine used once in a module is placed at its call site: the
//! `call` and the routine's `ret` (or the tail `jp`) go, as the inliner drops
//! the call to the last user of a static function. Only leaf routines qualify:
//! no stack, no calls, jumps to their own local labels, one `ret` at the end.

/// The module's assembly with each sole-use leaf routine of `runtime` expanded
/// where it is called.
pub fn inline_sole_uses(
    module: &str,
    runtime: &str,
) -> String {
    let mut text = module.to_owned();
    for (name, body) in leaf_routines(runtime) {
        let mentions = mentions(&text, &name);
        let [site] = mentions.as_slice() else { continue };
        let line = text.lines().nth(*site).expect("a mention is a line");
        let tail = match line.trim() {
            call if call == format!("call {name}") => false,
            jump if jump == format!("jp {name}") => true,
            _ => continue,
        };
        let expanded = expand(&name, &body, tail);
        text = text
            .lines()
            .enumerate()
            .fold(
                String::new(),
                |mut out, (number, line)| {
                    if number == *site {
                        out.push_str(&expanded)
                    } else {
                        out.push_str(line);
                        out.push('\n')
                    }
                    out
                },
            );
    }
    text
}

/// The numbers of the lines that name `symbol`, as a whole word.
fn mentions(
    text: &str,
    symbol: &str,
) -> Vec<usize> {
    let word = |c: char| c.is_alphanumeric() || c == '_';
    text.lines()
        .enumerate()
        .filter(|(_, line)| {
            let code = line.split(';').next().unwrap_or("");
            code.match_indices(symbol)
                .any(|(at, _)| !code[..at].ends_with(word) && !code[at + symbol.len()..].starts_with(word))
        })
        .map(|(number, _)| number)
        .collect()
}

/// `(label, lines)` of the runtime's routines that can be placed inline.
fn leaf_routines(runtime: &str) -> Vec<(String, Vec<String>)> {
    let mut routines: Vec<(String, Vec<String>)> = Vec::new();
    for line in runtime.lines() {
        let code = line.split(';').next().unwrap_or("").trim_end();
        if code.trim().is_empty() || code.trim_start().starts_with('.') {
            continue;
        }
        let global = code.split_once(':').filter(|(label, _)| {
            !code.starts_with(char::is_whitespace) && !label.ends_with('$') && !label.contains(char::is_whitespace)
        });
        match global {
            Some((label, rest)) => {
                routines.push((label.to_owned(), Vec::new()));
                if !rest.trim().is_empty() {
                    routines.last_mut().expect("just pushed").1.push(rest.to_owned());
                }
            }
            None => {
                if let Some((_, body)) = routines.last_mut() {
                    body.push(code.to_owned());
                }
            }
        }
    }
    routines.retain(|(_, body)| is_leaf(body));
    routines
}

/// The instruction on a body line, after any label.
fn instruction(line: &str) -> &str {
    match line.split_once(':') {
        Some((label, rest)) if !label.contains(char::is_whitespace) => rest.trim(),
        _ => line.trim(),
    }
}

fn is_leaf(body: &[String]) -> bool {
    let insts: Vec<&str> = body.iter().map(|line| instruction(line)).filter(|inst| !inst.is_empty()).collect();
    let local = |target: &str| target.trim().strip_suffix('$').is_some_and(|n| n.chars().all(|c| c.is_ascii_digit()));
    insts.last() == Some(&"ret")
        && insts.iter().filter(|inst| inst.starts_with("ret")).count() == 1
        && insts.iter().all(|inst| {
            let (op, args) = inst.split_once(char::is_whitespace).unwrap_or((inst, ""));
            match op {
                "ret" => true,
                "call" | "push" | "pop" | "rst" | "reti" | "retn" => false,
                "jp" | "jr" | "djnz" => local(args.rsplit(',').next().unwrap_or("")),
                _ => !args.split(',').any(|arg| matches!(arg.trim(), "sp" | "ix") || arg.contains("(ix")),
            }
        })
}

/// The routine's lines with local labels made unique to `name`; a call drops
/// the final `ret`.
fn expand(
    name: &str,
    body: &[String],
    tail: bool,
) -> String {
    let rename = |line: &str| {
        let mut out = String::new();
        let mut rest = line;
        while let Some(at) = rest.find('$') {
            let digits = rest[..at].chars().rev().take_while(char::is_ascii_digit).count();
            out.push_str(&rest[..at - digits]);
            out.push_str(&format!("{name}_{}", &rest[at - digits..at]));
            rest = &rest[at + 1..];
        }
        out.push_str(rest);
        out
    };
    let last = body.iter().rposition(|line| instruction(line) == "ret").expect("a leaf routine ends in ret");
    let mut out = String::new();
    for (number, line) in body.iter().enumerate() {
        let line = rename(line);
        let (label, inst) = match line.split_once(':') {
            Some((label, rest)) if !label.contains(char::is_whitespace) => (Some(label), rest.trim()),
            _ => (None, line.trim()),
        };
        if let Some(label) = label {
            out.push_str(&format!("{label}:\n"));
        }
        if !inst.is_empty() && !(number == last && !tail) {
            out.push_str(&format!("\t{inst}\n"));
        }
    }
    out
}
