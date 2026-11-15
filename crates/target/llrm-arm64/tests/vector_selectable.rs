//! A splat of a loaded scalar was selected as `LD1Rv4s`, which had no
//! instruction: `jpeg_single` at O2 failed to build ("machine opcode LD1Rv4s
//! has no arm64 instruction yet"). The sweep puts every vector operation the
//! selector picks from IR over every NEON type through the compiler and fails
//! on any machine opcode it selects but cannot emit.

use std::process::Command;

const HEAD: &str = "target triple = \"aarch64-unknown-linux-gnu\"\n\
target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32\"\n";

const MISSING: &str = "has no arm64 instruction yet";

/// DEVIATE VEC9 (owner llrm-arm64-vec, exit 2026-11-24): the 64-bit float
/// forms (`FADDv2f32`) are selected and not emitted: the 64-bit vector work.
/// Any other missing opcode fails the test.
const KNOWN_MISSING_SUFFIX: &str = "v2f32 has no";

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

/// One function over the vector type `{T}` with lane type `{E}`, lane count
/// `{N}`, loading `%a` and `%b` and storing the result `%r` at `%d`.
const INTEGER_BODIES: &[(&str, &str)] = &[
    ("add", "%r = add {T} %a, %b"),
    ("sub", "%r = sub {T} %a, %b"),
    ("mul", "%r = mul {T} %a, %b"),
    ("and", "%r = and {T} %a, %b"),
    ("or", "%r = or {T} %a, %b"),
    ("xor", "%r = xor {T} %a, %b"),
    ("not", "%r = xor {T} %a, splat ({E} -1)"),
    ("neg", "%r = sub {T} zeroinitializer, %a"),
    ("shl", "%r = shl {T} %a, %b"),
    ("lshr", "%r = lshr {T} %a, %b"),
    ("ashr", "%r = ashr {T} %a, %b"),
    ("shl_imm", "%r = shl {T} %a, splat ({E} 3)"),
    ("lshr_imm", "%r = lshr {T} %a, splat ({E} 3)"),
    ("ashr_imm", "%r = ashr {T} %a, splat ({E} 3)"),
    ("select", "%c = icmp slt {T} %a, %b\n%r = select <{N} x i1> %c, {T} %a, {T} %b"),
    ("icmp_zero", "%c = icmp sgt {T} %a, zeroinitializer\n%r = sext <{N} x i1> %c to {T}"),
    ("splat_load", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer"),
    ("splat_load_add", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%s = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer\n%r = add {T} %a, %s"),
    ("splat_arg", "%i = insertelement {T} poison, {E} %s, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer"),
    ("splat_const", "%r = add {T} %a, splat ({E} 7)"),
    ("insert_lane", "%r = insertelement {T} %a, {E} %s, i64 1"),
    ("insert_lane_load", "%x = load {E}, ptr %p, align 8\n%r = insertelement {T} %a, {E} %x, i64 1"),
    ("extract_lane", "%x = extractelement {T} %a, i64 1\nstore {E} %x, ptr %q, align 8\n%r = add {T} %a, %b"),
    ("abs", "%r = call {T} @llvm.abs.{V}({T} %a, i1 false)"),
    ("smin", "%r = call {T} @llvm.smin.{V}({T} %a, {T} %b)"),
    ("smax", "%r = call {T} @llvm.smax.{V}({T} %a, {T} %b)"),
    ("umin", "%r = call {T} @llvm.umin.{V}({T} %a, {T} %b)"),
    ("umax", "%r = call {T} @llvm.umax.{V}({T} %a, {T} %b)"),
    ("reduce_add", "%x = call {E} @llvm.vector.reduce.add.{V}({T} %a)\nstore {E} %x, ptr %q, align 8\n%r = add {T} %a, %b"),
    ("ctpop", "%r = call {T} @llvm.ctpop.{V}({T} %a)"),
];

const FLOAT_BODIES: &[(&str, &str)] = &[
    ("fadd", "%r = fadd {T} %a, %b"),
    ("fsub", "%r = fsub {T} %a, %b"),
    ("fmul", "%r = fmul {T} %a, %b"),
    ("fdiv", "%r = fdiv {T} %a, %b"),
    ("fneg", "%r = fneg {T} %a"),
    ("fabs", "%r = call {T} @llvm.fabs.{V}({T} %a)"),
    ("fsqrt", "%r = call {T} @llvm.sqrt.{V}({T} %a)"),
    ("fma", "%r = call {T} @llvm.fma.{V}({T} %a, {T} %b, {T} %a)"),
    ("fcmp_select", "%c = fcmp olt {T} %a, %b\n%r = select <{N} x i1> %c, {T} %a, {T} %b"),
    ("fcmp_zero", "%c = fcmp ogt {T} %a, zeroinitializer\n%r = select <{N} x i1> %c, {T} %a, {T} %b"),
    ("splat_load", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer"),
    ("splat_load_mul", "%x = load {E}, ptr %p, align 8\n%i = insertelement {T} poison, {E} %x, i64 0\n%s = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer\n%r = fmul {T} %a, %s"),
    ("splat_arg", "%i = insertelement {T} poison, {E} %s, i64 0\n%r = shufflevector {T} %i, {T} poison, <{N} x i32> zeroinitializer"),
    ("extract_lane", "%x = extractelement {T} %a, i64 1\nstore {E} %x, ptr %q, align 8\n%r = fadd {T} %a, %b"),
    ("minnum", "%r = call {T} @llvm.minnum.{V}({T} %a, {T} %b)"),
    ("maxnum", "%r = call {T} @llvm.maxnum.{V}({T} %a, {T} %b)"),
];

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
    let body = body.replace("{T}", &vector).replace("{E}", element).replace("{N}", &lanes.to_string()).replace("{V}", &tag);
    let scalar = element;
    let decls = ["abs", "smin", "smax", "umin", "umax", "ctpop", "fabs", "sqrt", "fma", "minnum", "maxnum", "vector.reduce.add"]
        .into_iter()
        .filter(|name| body.contains(&format!("@llvm.{name}.")))
        .map(|name| {
            let signature = match name {
                "abs" => format!("{vector}, i1"),
                "fma" => format!("{vector}, {vector}, {vector}"),
                "vector.reduce.add" => vector.clone(),
                "smin" | "smax" | "umin" | "umax" | "minnum" | "maxnum" => format!("{vector}, {vector}"),
                _ => vector.clone(),
            };
            let result = if name == "vector.reduce.add" { scalar.to_string() } else { vector.clone() };
            format!("declare {result} @llvm.{name}.{tag}({signature})\n")
        })
        .collect::<String>();
    format!(
        "{HEAD}{decls}define void @f(ptr %d, ptr %a_p, ptr %b_p, ptr %p, ptr %q, {scalar} %s) {{\n\
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

/// Every vector operation over every NEON type that selects a machine opcode
/// selects one with an instruction. A refusal for another reason (a legalizer
/// gap) is not this test's business.
#[test]
fn every_selected_vector_opcode_has_an_instruction() {
    let mut missing = Vec::new();
    for (lanes, element, float) in TYPES {
        let bodies = if float { FLOAT_BODIES } else { INTEGER_BODIES };
        for (name, body) in bodies {
            let source = function(lanes, element, body);
            if let Some(said) = complaint(&source) {
                if said.contains(MISSING) && !said.contains(KNOWN_MISSING_SUFFIX) {
                    missing.push(format!("{name} <{lanes} x {element}>: {said}"));
                }
            }
        }
    }
    assert!(missing.is_empty(), "selected but not emitted:\n{}", missing.join("\n"));
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
