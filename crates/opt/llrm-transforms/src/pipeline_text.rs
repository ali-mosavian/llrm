//! The pass pipeline text of `opt -passes=`: `a,b(c,d),e<x;y>`. `parse` reads
//! the syntax (`PassBuilder::parsePassPipeline`); `build` adds the passes this
//! crate has to a manager and names the first it does not have.

use llrm_mir::passes::PassManager;

/// One pipeline element: a pass with its `<parameters>`, or an adaptor
/// (`function`, `loop`, ...) around a nested pipeline.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Element {
    Pass { name: String, params: Option<String> },
    Nested { name: String, inner: Vec<Element> },
}

/// A pass or option this crate has not ported: `opt` stays quiet about what it
/// does not know, the test runner counts it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsupported(pub String);

/// The elements of `text`; an error names the offset of the first bad byte.
pub fn parse(text: &str) -> Result<Vec<Element>, String> {
    let bytes = text.as_bytes();
    let (elements, end) = sequence(text, 0)?;
    if end != bytes.len() {
        return Err(format!("unexpected `{}` at {end}", &text[end..=end]));
    }
    Ok(elements)
}

fn sequence(
    text: &str,
    mut at: usize,
) -> Result<(Vec<Element>, usize), String> {
    let bytes = text.as_bytes();
    let mut elements = Vec::new();
    if at >= bytes.len() || bytes[at] == b')' {
        return Ok((elements, at));
    }
    loop {
        let start = at;
        while at < bytes.len() && !matches!(bytes[at], b',' | b'(' | b')' | b'<') {
            at += 1;
        }
        let name = text[start..at].trim().to_owned();
        if name.is_empty() {
            return Err(format!("a pass name is missing at {start}"));
        }
        match bytes.get(at) {
            Some(b'<') => {
                let close = text[at..].find('>').ok_or_else(|| format!("`<` at {at} is not closed"))? + at;
                elements.push(Element::Pass { name, params: Some(text[at + 1..close].to_owned()) });
                at = close + 1;
            }
            Some(b'(') => {
                let (inner, end) = sequence(text, at + 1)?;
                if bytes.get(end) != Some(&b')') {
                    return Err(format!("`(` at {at} is not closed"));
                }
                elements.push(Element::Nested { name, inner });
                at = end + 1;
            }
            _ => elements.push(Element::Pass { name, params: None }),
        }
        if bytes.get(at) == Some(&b',') {
            at += 1;
        } else {
            return Ok((elements, at));
        }
    }
}

/// The adaptors whose nested passes run in the order written, as the function
/// passes here do.
const ADAPTORS: &[&str] = &["module", "function", "loop", "loop-mssa"];

/// Adds the passes of `elements` to `manager`.
pub fn build(
    elements: &[Element],
    manager: &mut PassManager,
) -> Result<(), Unsupported> {
    for element in elements {
        match element {
            Element::Nested { name, inner } if ADAPTORS.contains(&name.as_str()) => build(inner, manager)?,
            Element::Nested { name, .. } => return Err(Unsupported(format!("pass adaptor {name}"))),
            Element::Pass { name, params } => add(name, params.as_deref(), manager)?,
        }
    }
    Ok(())
}

fn add(
    name: &str,
    params: Option<&str>,
    manager: &mut PassManager,
) -> Result<(), Unsupported> {
    let unsupported = |what: &str| Unsupported(what.to_owned());
    match (name, params) {
        // Analyses the passes here compute on demand.
        ("require" | "invalidate", Some(_)) | ("verify", None) => {}
        ("loop-reduce", None) => manager.add(crate::lsr_pass::LoopStrengthReduce { size: false }),
        ("indvars", None) => manager.add(crate::indvars::IndVars { lftr: true, size: false }),
        ("indvars", Some("no-lftr")) => manager.add(crate::indvars::IndVars { lftr: false, size: false }),
        ("loop-rotate", None) => manager.add(crate::looprotate::LoopRotate { duplicate_headers: true, size: false }),
        ("loop-deletion", None) => manager.add(crate::loopdeletion::LoopDeletion),
        ("loop-simplify", None) => manager.add(crate::loopsimplify::LoopSimplify),
        ("loop-instsimplify", None) => manager.add(crate::loopinstsimplify::LoopInstSimplify),
        ("lcssa", None) => manager.add(crate::lcssa::LoopClosedSSA),
        ("dce", None) => manager.add(crate::dead::Dead),
        ("adce", None) => manager.add(crate::adce::Adce),
        ("instcombine", None) => manager.add(crate::combine_pass::Combine { enabled: true }),
        ("simplifycfg", None) => manager.add(crate::simplifycfg::SimplifyCfg::default()),
        (_, Some(params)) => return Err(unsupported(&format!("{name}<{params}>"))),
        _ => return Err(unsupported(name)),
    }
    Ok(())
}

/// The pass a legacy `-loop-reduce` style flag names, as `-passes=` text.
pub fn legacy(flag: &str) -> Option<&'static str> {
    const NAMES: &[&str] = &[
        "loop-reduce",
        "indvars",
        "loop-rotate",
        "loop-deletion",
        "loop-simplify",
        "loop-instsimplify",
        "lcssa",
        "dce",
        "adce",
        "instcombine",
        "simplifycfg",
    ];
    NAMES.iter().copied().find(|name| *name == flag)
}

#[cfg(test)]
#[path = "pipeline_text_tests.rs"]
mod tests;
