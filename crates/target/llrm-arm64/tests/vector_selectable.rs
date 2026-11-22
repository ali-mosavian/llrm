//! A splat of a loaded scalar was selected as `LD1Rv4s`, which had no
//! instruction: `jpeg_single` at O2 failed to build ("machine opcode LD1Rv4s
//! has no arm64 instruction yet"). The sweep puts every vector operation the
//! frontend can emit over every NEON type through the compiler and fails on a
//! panic and on any refusal not listed in `REFUSED`.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

/// `(lane count, lane type)` of each NEON vector and whether its lanes are floats.
const TYPES: [(u32, &str, bool); 10] = [
    (16, "i8", false),
    (8, "i16", false),
    (4, "i32", false),
    (2, "i64", false),
    (8, "i8", false),
    (4, "i16", false),
    (2, "i32", false),
    (4, "float", true),
    (2, "double", true),
    (2, "float", true),
];

/// One operation: a name, the body over the vector type `{T}` (lane type `{E}`,
/// lane count `{N}`, intrinsic suffix `{V}`, double-width lane `{W}`, integer
/// lane of the same width `{I}`) loading `%a` and `%b` and leaving `%r`, and
/// the lane types it applies to.
type Operation = (&'static str, &'static str, fn(&str) -> bool);

fn any(_: &str) -> bool {
    true
}
fn not_i64(element: &str) -> bool {
    element != "i64"
}
fn not_i8(element: &str) -> bool {
    element != "i8"
}
fn not_i8_i64(element: &str) -> bool {
    !matches!(element, "i8" | "i64")
}

const INTEGER_BODIES: &[Operation] = &[
    ("add", "%r = add {T} %a, %b", any),
    ("sub", "%r = sub {T} %a, %b", any),
    ("mul", "%r = mul {T} %a, %b", any),
    ("and", "%r = and {T} %a, %b", any),
    ("or", "%r = or {T} %a, %b", any),
    ("xor", "%r = xor {T} %a, %b", any),
    ("not", "%r = xor {T} %a, splat ({E} -1)", any),
    ("neg", "%r = sub {T} zeroinitializer, %a", any),
    ("shl", "%r = shl {T} %a, %b", any),
    ("lshr", "%r = lshr {T} %a, %b", any),
    ("ashr", "%r = ashr {T} %a, %b", any),
    ("shl_imm", "%r = shl {T} %a, splat ({E} 3)", any),
    ("lshr_imm", "%r = lshr {T} %a, splat ({E} 3)", any),
    ("ashr_imm", "%r = ashr {T} %a, splat ({E} 3)", any),
    ("select", "%c = icmp slt {T} %a, %b\n%r = select <{N} x i1> %c, {T} %a, {T} %b", any),
    ("icmp_zero", "%c = icmp sgt {T} %a, zeroinitializer\n%r = sext <{N} x i1> %c to {T}", any),
    ("splat_load", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer", any),
    ("splat_load_add", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%s2 = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer\n%r = add {T} %a, %s2", any),
    ("splat_arg", "%i = insertelement {T} poison, {E} %s, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer", any),
    ("splat_const", "%r = add {T} %a, splat ({E} 7)", any),
    ("insert_lane", "%r = insertelement {T} %a, {E} %s, i64 1", any),
    ("insert_lane_load", "%x = load {E}, ptr %p, align 8\n%r = insertelement {T} %a, {E} %x, i64 1", any),
    ("extract_lane", "%x = extractelement {T} %a, i64 1\nstore {E} %x, ptr %q, align 8\n%r = add {T} %a, %b", any),
    ("abs", "%r = call {T} @llvm.abs.{V}({T} %a, i1 false)", any),
    ("smin", "%r = call {T} @llvm.smin.{V}({T} %a, {T} %b)", any),
    ("smax", "%r = call {T} @llvm.smax.{V}({T} %a, {T} %b)", any),
    ("umin", "%r = call {T} @llvm.umin.{V}({T} %a, {T} %b)", any),
    ("umax", "%r = call {T} @llvm.umax.{V}({T} %a, {T} %b)", any),
    ("reduce_add", "%x = call {E} @llvm.vector.reduce.add.{V}({T} %a)\nstore {E} %x, ptr %q, align 8\n%r = add {T} %a, %b", any),
    ("ctpop", "%r = call {T} @llvm.ctpop.{V}({T} %a)", any),
    ("ctlz", "%r = call {T} @llvm.ctlz.{V}({T} %a, i1 false)", any),
    ("cttz", "%r = call {T} @llvm.cttz.{V}({T} %a, i1 false)", any),
    ("bitreverse", "%r = call {T} @llvm.bitreverse.{V}({T} %a)", any),
    ("bswap", "%r = call {T} @llvm.bswap.{V}({T} %a)", not_i8),
    ("uadd_sat", "%r = call {T} @llvm.uadd.sat.{V}({T} %a, {T} %b)", any),
    ("usub_sat", "%r = call {T} @llvm.usub.sat.{V}({T} %a, {T} %b)", any),
    ("sadd_sat", "%r = call {T} @llvm.sadd.sat.{V}({T} %a, {T} %b)", any),
    ("ssub_sat", "%r = call {T} @llvm.ssub.sat.{V}({T} %a, {T} %b)", any),
    ("zext_trunc", "%w = zext {T} %a to <{N} x {W}>\n%r = trunc <{N} x {W}> %w to {T}", not_i64),
    ("sext_trunc", "%w = sext {T} %a to <{N} x {W}>\n%r = trunc <{N} x {W}> %w to {T}", not_i64),
    ("uitofp_fptoui", "%w = uitofp {T} %a to <{N} x {F}>\n%r = fptoui <{N} x {F}> %w to {T}", not_i8_i64),
    ("sitofp_fptosi", "%w = sitofp {T} %a to <{N} x {F}>\n%r = fptosi <{N} x {F}> %w to {T}", not_i8_i64),
];

const FLOAT_BODIES: &[Operation] = &[
    ("fadd", "%r = fadd {T} %a, %b", any),
    ("fsub", "%r = fsub {T} %a, %b", any),
    ("fmul", "%r = fmul {T} %a, %b", any),
    ("fdiv", "%r = fdiv {T} %a, %b", any),
    ("fneg", "%r = fneg {T} %a", any),
    ("fabs", "%r = call {T} @llvm.fabs.{V}({T} %a)", any),
    ("fsqrt", "%r = call {T} @llvm.sqrt.{V}({T} %a)", any),
    ("fmuladd", "%r = call {T} @llvm.fmuladd.{V}({T} %a, {T} %b, {T} %a)", any),
    ("fcmp_select", "%c = fcmp olt {T} %a, %b\n%r = select <{N} x i1> %c, {T} %a, {T} %b", any),
    ("fcmp_zero", "%c = fcmp ogt {T} %a, zeroinitializer\n%r = select <{N} x i1> %c, {T} %a, {T} %b", any),
    ("splat_load", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer", any),
    ("splat_load_mul", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%s2 = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer\n%r = fmul {T} %a, %s2", any),
    ("splat_arg", "%i = insertelement {T} poison, {E} %s, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer", any),
    ("extract_lane", "%x = extractelement {T} %a, i64 1\nstore {E} %x, ptr %q, align 8\n%r = fadd {T} %a, %b", any),
    ("copysign", "%r = call {T} @llvm.copysign.{V}({T} %a, {T} %b)", any),
    ("floor", "%r = call {T} @llvm.floor.{V}({T} %a)", any),
    ("ceil", "%r = call {T} @llvm.ceil.{V}({T} %a)", any),
    ("trunc", "%r = call {T} @llvm.trunc.{V}({T} %a)", any),
    ("rint", "%r = call {T} @llvm.rint.{V}({T} %a)", any),
    ("nearbyint", "%r = call {T} @llvm.nearbyint.{V}({T} %a)", any),
    ("round", "%r = call {T} @llvm.round.{V}({T} %a)", any),
    ("sitofp_fptosi", "%w = fptosi {T} %a to <{N} x {I}>\n%r = sitofp <{N} x {I}> %w to {T}", any),
    ("uitofp_fptoui", "%w = fptoui {T} %a to <{N} x {I}>\n%r = uitofp <{N} x {I}> %w to {T}", any),
];

/// The declarations of the intrinsics a body calls, read off its `call` lines.
fn declarations(body: &str) -> String {
    let mut seen = Vec::new();
    for line in body.lines() {
        let Some(call) = line.split(" = call ").nth(1).or_else(|| line.strip_prefix("call ")) else { continue };
        let Some((result, rest)) = call.split_once(" @") else { continue };
        let Some((name, arguments)) = rest.split_once('(') else { continue };
        let types = arguments
            .trim_end_matches(')')
            .split(", ")
            .map(|argument| argument.rsplit_once(' ').map_or(argument, |(ty, _)| ty))
            .collect::<Vec<_>>()
            .join(", ");
        let declaration = format!("declare {result} @{name}({types})\n");
        if !seen.contains(&declaration) {
            seen.push(declaration);
        }
    }
    seen.concat()
}

fn function(
    lanes: u32,
    element: &str,
    body: &str,
) -> String {
    let vector = format!("<{lanes} x {element}>");
    let tag = format!("v{lanes}{}", match element {
        "float" => "f32",
        "double" => "f64",
        other => other,
    });
    let double_width = match element {
        "i8" => "i16",
        "i16" => "i32",
        "i32" => "i64",
        other => other,
    };
    let (integer, float) = match element {
        "float" => ("i32", "float"),
        "double" => ("i64", "double"),
        "i16" => ("i16", "float"),
        _ => ("i32", "float"),
    };
    let body = body
        .replace("{T}", &vector)
        .replace("{E}", element)
        .replace("{N}", &lanes.to_string())
        .replace("{V}", &tag)
        .replace("{W}", double_width)
        .replace("{I}", integer)
        .replace("{F}", float);
    let decls = declarations(&body);
    format!(
        "{HEAD}{decls}define void @f(ptr %d, ptr %a_p, ptr %b_p, ptr %p, ptr %q, {element} %s) {{\n\
         %a = load {vector}, ptr %a_p, align 16\n\
         %b = load {vector}, ptr %b_p, align 16\n\
         {body}\n\
         store {vector} %r, ptr %d, align 16\n\
         ret void\n}}\n"
    )
}

/// The compiler's complaint about `source`, if it has one.
fn complaint(source: &str) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
    std::fs::write(&input, source).unwrap();
    let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
        .args(["-S", "-O2", "-o"])
        .arg(&output)
        .arg(&input)
        .output()
        .unwrap();
    (!said.status.success()).then(|| String::from_utf8_lossy(&said.stderr).trim().to_string())
}

/// DEVIATE VEC10 (owner llrm-arm64-vec, exit 2026-11-24): the operations the
/// compiler refuses, as `(operation, the refusal's text)`: a row covers every
/// vector type that fails that way. A refusal not listed fails the test, and so
/// does a row nothing needs. A panic is never listed.
const REFUSED: &[(&str, &str)] = &[
    // The shifts by a vector of amounts (`USHL`/`SSHL` by a negated amount).
    ("shl", "custom lowering of Shl"),
    ("lshr", "custom lowering of Srl"),
    ("ashr", "custom lowering of Sra"),
    // `CLZ`, `RBIT` and the saturating arithmetic have no instruction; `CNT`
    // is bytes only, so `ctpop`, `cttz` and `bitreverse` of wider lanes have no pattern.
    ("ctlz", "CannotSelect(\"CtLz\")"),
    ("cttz", "CannotSelect(\"CtTz\")"),
    ("ctpop", "CannotSelect(\"CtPop\")"),
    ("bitreverse", "CannotSelect(\"BitReverse\")"),
    ("bswap", "bswap must be an even number of bytes"),
    // The conversions of 16-bit lanes.
    ("uitofp_fptoui", "CannotSelect(\"FpToUInt\")"),
    ("sitofp_fptosi", "CannotSelect(\"FpToSInt\")"),
    // The float operations with no vector form yet.
    ("copysign", "CannotSelect(\"FCopySign\")"),
    ("floor", "a rounding intrinsic without its library call"),
    ("ceil", "a rounding intrinsic without its library call"),
    ("trunc", "a rounding intrinsic without its library call"),
    ("nearbyint", "a rounding intrinsic without its library call"),
    ("round", "a rounding intrinsic without its library call"),
    ("rint", "machine opcode FRINTXv"),
    ("splat_load_mul", "machine opcode FMULv"),
];

/// Every vector operation over every NEON type either compiles or is listed
/// in `REFUSED`: a selected opcode without an instruction, a legalizer gap, a
/// type the target does not take.
#[test]
fn every_vector_operation_compiles_or_is_listed() {
    let mut refused = Vec::new();
    for (lanes, element, float) in TYPES {
        let bodies = if float { FLOAT_BODIES } else { INTEGER_BODIES };
        for (name, body, applies) in bodies {
            if applies(element) {
                if let Some(said) = complaint(&function(lanes, element, body)) {
                    refused.push((*name, format!("<{lanes} x {element}>"), said));
                }
            }
        }
    }
    let panics: Vec<_> = refused.iter().filter(|(_, _, said)| said.contains("panicked")).collect();
    assert!(panics.is_empty(), "the compiler panicked:\n{panics:#?}");
    let listed = |name: &str, said: &str| REFUSED.iter().any(|(op, text)| *op == name && said.contains(text));
    let unlisted: Vec<_> = refused.iter().filter(|(name, _, said)| !listed(name, said)).collect();
    let stale: Vec<_> = REFUSED
        .iter()
        .filter(|(op, text)| !refused.iter().any(|(name, _, said)| name == op && said.contains(text)))
        .collect();
    assert!(
        unlisted.is_empty() && stale.is_empty(),
        "refused and not listed:\n{unlisted:#?}\nlisted and no longer refused:\n{stale:#?}",
    );
}

/// A splat of a loaded scalar is one `ld1r` on every 128-bit arrangement (the
/// 64-bit `build_vector` is another lane's work).
#[test]
fn a_splat_of_a_load_is_ld1r() {
    for (lanes, element, _) in TYPES.into_iter().filter(|(lanes, element, _)| lanes * if *element == "double" || *element == "i64" { 64 } else if *element == "float" || *element == "i32" { 32 } else if *element == "i16" { 16 } else { 8 } == 128) {
        let source = function(lanes, element, "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer");
        let dir = tempfile::tempdir().unwrap();
        let (input, output) = (dir.path().join("t.ll"), dir.path().join("t.s"));
        std::fs::write(&input, &source).unwrap();
        let said = Command::new(env!("CARGO_BIN_EXE_llrm-arm64-llc"))
            .args(["-S", "-O2", "-o"])
            .arg(&output)
            .arg(&input)
            .output()
            .unwrap();
        assert!(said.status.success(), "<{lanes} x {element}>: {}", String::from_utf8_lossy(&said.stderr));
        let text = std::fs::read_to_string(&output).unwrap();
        assert!(text.contains("ld1r"), "<{lanes} x {element}> has no ld1r:\n{text}");
    }
}

/// An `insertelement` of a loaded scalar is `ld1 { v.s }[1], [x]`. Its source
/// vector was never copied into the destination, so the live-range
/// computation panicked ("a unique value") on every lane width.
#[test]
fn a_lane_load_into_a_vector_compiles() {
    for (lanes, element, _) in TYPES.into_iter().filter(|(_, _, float)| !float) {
        let source = function(lanes, element, "%x = load {E}, ptr %p, align 8\n%r = insertelement {T} %a, {E} %x, i64 1");
        assert_eq!(complaint(&source), None, "<{lanes} x {element}>");
    }
}
