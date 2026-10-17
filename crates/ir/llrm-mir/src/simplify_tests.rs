use crate::context::ConstantKind;
use crate::module::{Module, Operand};
use crate::{parse, simplify};

/// What `simplify_instruction` makes of `%name` in `@f`, spelled `%x` for a
/// value, `N` for a number or `poison`; None where it finds nothing.
fn simplified(
    text: &str,
    name: &str,
) -> Option<String> {
    let mut module: Module = parse::module(text).unwrap_or_else(|why| panic!("{why:?}\n{text}"));
    let id = module.named("f").expect("@f");
    let crate::module::GlobalKind::Function(function) = &module.globals[id.0 as usize].kind else {
        panic!("a function")
    };
    let function = function.clone();
    let inst = function
        .walk()
        .map(|(_, inst)| inst)
        .find(|&inst| {
            function.instruction(inst).result.is_some_and(|result| function.value(result).name.as_deref() == Some(name))
        })
        .expect("a named instruction");
    let found = simplify::simplify_instruction(
        &mut module.context,
        &crate::datalayout::DataLayout::default(),
        &function,
        inst,
    )?;
    Some(match found {
        Operand::Value(value) => format!("%{}", function.value(value).name.clone().unwrap_or_default()),
        Operand::Constant(id) => match module.context.get(id).kind {
            ConstantKind::Int(bits) => bits.to_string(),
            ConstantKind::Poison => "poison".to_owned(),
            ConstantKind::Null | ConstantKind::Zero => "0".to_owned(),
            ref other => format!("{other:?}"),
        },
        Operand::Block(_) => unreachable!("an instruction is never a block"),
    })
}

/// A body of `i16`s: `body` after the parameters `%x`, `%y`.
fn of(body: &str) -> String {
    format!("define i16 @f(i16 %x, i16 %y, i1 %c) {{\nb0:\n{body}  ret i16 %x\n}}\n")
}

fn check(
    body: &str,
    name: &str,
    expected: Option<&str>,
) {
    assert_eq!(simplified(&of(body), name).as_deref(), expected, "{body}");
}

#[test]
fn additive_identities_and_negations() {
    check("  %r = add i16 %x, 0\n", "r", Some("%x"));
    check("  %r = add i16 0, %x\n", "r", Some("%x"));
    check("  %n = sub i16 0, %x\n  %r = add i16 %x, %n\n", "r", Some("0"));
    check("  %d = sub i16 %y, %x\n  %r = add i16 %x, %d\n", "r", Some("%y"));
    check("  %nx = xor i16 %x, -1\n  %r = add i16 %x, %nx\n", "r", Some("65535"));
    check("  %r = add i16 %x, %y\n", "r", None);
    check("  %r = sub i16 %x, 0\n", "r", Some("%x"));
    check("  %r = sub i16 %x, %x\n", "r", Some("0"));
    check("  %a = add i16 %x, %y\n  %r = sub i16 %a, %y\n", "r", Some("%x"));
    check("  %r = sub i16 %x, %y\n", "r", None);
}

#[test]
fn multiplicative_and_division_cases() {
    check("  %r = mul i16 %x, 1\n", "r", Some("%x"));
    check("  %r = mul i16 1, %x\n", "r", Some("%x"));
    check("  %r = mul i16 %x, 0\n", "r", Some("0"));
    check("  %r = udiv i16 %x, 0\n", "r", Some("poison"));
    check("  %r = udiv i16 0, %y\n", "r", Some("0"));
    check("  %r = sdiv i16 %x, %x\n", "r", Some("1"));
    check("  %r = urem i16 %x, %x\n", "r", Some("0"));
    check("  %m = mul i16 %x, %y\n  %r = udiv exact i16 %m, %y\n", "r", None);
    check("  %m = mul nuw i16 %x, %y\n  %r = udiv i16 %m, %y\n", "r", Some("%x"));
    check("  %m = mul nuw i16 %x, %y\n  %r = urem i16 %m, %y\n", "r", Some("0"));
}

#[test]
fn shifts() {
    check("  %r = shl i16 %x, 0\n", "r", Some("%x"));
    check("  %r = shl i16 0, %x\n", "r", Some("0"));
    check("  %r = lshr i16 %x, 16\n", "r", Some("poison"));
    check("  %r = lshr i16 %x, %x\n", "r", Some("0"));
    check("  %s = shl nuw i16 %x, %y\n  %r = lshr i16 %s, %y\n", "r", Some("%x"));
    check("  %s = shl nsw i16 %x, %y\n  %r = ashr i16 %s, %y\n", "r", Some("%x"));
    check("  %r = ashr i16 -1, %y\n", "r", Some("65535"));
}

#[test]
fn logic() {
    check("  %r = and i16 %x, %x\n", "r", Some("%x"));
    check("  %r = and i16 %x, 0\n", "r", Some("0"));
    check("  %r = and i16 %x, -1\n", "r", Some("%x"));
    check("  %n = xor i16 %x, -1\n  %r = and i16 %x, %n\n", "r", Some("0"));
    check("  %o = or i16 %x, %y\n  %r = and i16 %o, %x\n", "r", Some("%x"));
    check("  %r = or i16 %x, -1\n", "r", Some("65535"));
    check("  %r = or i16 %x, 0\n", "r", Some("%x"));
    check("  %n = xor i16 %x, -1\n  %r = or i16 %x, %n\n", "r", Some("65535"));
    check("  %a = and i16 %x, %y\n  %r = or i16 %x, %a\n", "r", Some("%x"));
    check("  %r = xor i16 %x, %x\n", "r", Some("0"));
    check("  %r = xor i16 %x, 0\n", "r", Some("%x"));
    check("  %r = xor i16 %x, %y\n", "r", None);
    check("  %a = and i16 %x, 255\n  %r = and i16 %a, 255\n", "r", Some("%a"));
    check("  %s = shl i16 %x, 8\n  %r = and i16 %s, 65280\n", "r", Some("%s"));
}

#[test]
fn select_phi_cast_and_compare() {
    check("  %r = select i1 true, i16 %x, i16 %y\n", "r", Some("%x"));
    check("  %r = select i1 false, i16 %x, i16 %y\n", "r", Some("%y"));
    check("  %r = select i1 %c, i16 %x, i16 %x\n", "r", Some("%x"));
    check("  %r = select i1 %c, i1 true, i1 false\n", "r", Some("%c"));
    let phi = "define i16 @f(i16 %x, i1 %c) {\nb0:\n  br i1 %c, label %a, label %b\na:\n  br label %j\nb:\n  br label %j\nj:\n  %r = phi i16 [ %x, %a ], [ %x, %b ]\n  ret i16 %r\n}\n";
    assert_eq!(simplified(phi, "r").as_deref(), Some("%x"));
    check("  %w = zext i16 %x to i32\n  %r = trunc i32 %w to i16\n", "r", Some("%x"));
    check("  %w = sext i16 %x to i32\n  %r = trunc i32 %w to i16\n", "r", Some("%x"));
    check("  %t = trunc i16 %x to i8\n  %r = zext i8 %t to i16\n", "r", None);
    check("  %r = icmp eq i16 %x, %x\n", "r", Some("1"));
    check("  %r = icmp ult i16 %x, %x\n", "r", Some("0"));
    check("  %r = icmp ult i16 %x, 0\n", "r", Some("0"));
    check("  %r = icmp uge i16 %x, 0\n", "r", Some("1"));
    check("  %w = zext i16 %x to i32\n  %v = zext i16 %y to i32\n  %r = icmp ult i32 %w, %v\n", "r", None);
}

/// The compares the zero-half stand-in missed, now answered from the real
/// ValueTracking: a mask is below its source, a shift amount known to be the
/// width is poison, and `lshr` bounds a range.
#[test]
fn compares_answered_from_value_tracking() {
    check("  %a = and i16 %x, %y\n  %c1 = icmp uge i16 %x, %a\n", "c1", Some("1"));
    check("  %o = or i16 %x, %y\n  %c1 = icmp ult i16 %o, %x\n", "c1", Some("0"));
    check("  %s = lshr i16 %x, 3\n  %c1 = icmp ult i16 %s, 8192\n", "c1", Some("1"));
    check("  %o = or i16 %x, 16\n  %c1 = icmp eq i16 %o, 0\n", "c1", Some("0"));
    check("  %r = shl i16 %x, 16\n", "r", Some("poison"));
}

/// Differential check against `opt -passes=instsimplify` (not part of the
/// gate): `LLRM_SIMPLIFY_CASES=in.ll LLRM_SIMPLIFY_OUT=out.txt cargo test -p
/// llrm-mir --lib simplify_cases -- --ignored`. Each function `@fN` has a
/// target `%t`; the output says, for each, whether `simplify_instruction` found
/// a value for it.
#[test]
#[ignore]
fn simplify_cases() {
    let (Some(input), Some(output)) = (std::env::var_os("LLRM_SIMPLIFY_CASES"), std::env::var_os("LLRM_SIMPLIFY_OUT"))
    else {
        return;
    };
    let text = std::fs::read_to_string(input).expect("cases");
    let mut module: Module = parse::module(&text).unwrap_or_else(|why| panic!("{why:?}"));
    let mut lines = String::new();
    for at in 0..module.globals.len() {
        let crate::module::GlobalKind::Function(function) = &module.globals[at].kind else { continue };
        let function = function.clone();
        let Some(inst) = function.walk().map(|(_, inst)| inst).find(|&inst| {
            function.instruction(inst).result.is_some_and(|result| function.value(result).name.as_deref() == Some("t"))
        }) else {
            continue;
        };
        let found = simplify::simplify_instruction(
            &mut module.context,
            &crate::datalayout::DataLayout::default(),
            &function,
            inst,
        )
        .map(|value| match value {
            Operand::Value(value) => format!("%{}", function.value(value).name.clone().unwrap_or_default()),
            Operand::Constant(id) => match module.context.get(id).kind {
                ConstantKind::Int(bits) => bits.to_string(),
                ConstantKind::Poison => "poison".to_owned(),
                ConstantKind::Null | ConstantKind::Zero => "0".to_owned(),
                ref other => format!("{other:?}"),
            },
            Operand::Block(_) => unreachable!("an instruction is never a block"),
        });
        lines += &format!(
            "{} {}\n",
            module.globals[at].name.clone().unwrap_or_default(),
            found.map_or("none".to_owned(), |value| format!("simplified {value}"))
        );
    }
    std::fs::write(output, lines).expect("out");
}

/// `instsimplify` on the body `pipeline.body` of each module in a list, as
/// LLVM's pass does it (delete the trivially dead, replace what simplifies,
/// until nothing does), then the opcodes left: the differential on real pass
/// inputs. `LLRM_SIMPLIFY_FILES=list.txt LLRM_SIMPLIFY_OUT=out.tsv cargo test
/// -p llrm-mir --lib simplify_dumps -- --ignored`.
#[test]
#[ignore]
fn simplify_dumps() {
    let (Some(list), Some(output)) = (std::env::var_os("LLRM_SIMPLIFY_FILES"), std::env::var_os("LLRM_SIMPLIFY_OUT"))
    else {
        return;
    };
    let mut lines = String::new();
    for path in std::fs::read_to_string(list).expect("list").lines() {
        let text = std::fs::read_to_string(path).expect("module");
        let Ok(mut module) = parse::module(&text) else {
            lines += &format!("{path}\tunparsed\n");
            continue;
        };
        let Some((context, function)) = module.function_mut("pipeline.body") else {
            lines += &format!("{path}\tno body\n");
            continue;
        };
        loop {
            let mut changed = false;
            for inst in function.walk().map(|(_, inst)| inst).collect::<Vec<_>>() {
                if function.parent(inst).is_none() {
                    continue;
                }
                let instruction = function.instruction(inst);
                let Some(result) = instruction.result else { continue };
                let unused = function.users(result).is_empty();
                use crate::opcode::Opcode;
                let pure = matches!(
                    instruction.opcode,
                    Opcode::Binary(_)
                        | Opcode::Cast(_)
                        | Opcode::ICmp(_)
                        | Opcode::FCmp(_)
                        | Opcode::Select
                        | Opcode::Phi
                        | Opcode::GetElementPtr { .. }
                        | Opcode::Alloca { .. }
                        | Opcode::FNeg
                        | Opcode::Freeze
                ) || matches!(instruction.opcode, Opcode::Load { volatile: false, .. });
                if unused && pure {
                    function.erase(inst).expect("unused");
                    changed = true;
                    continue;
                }
                if let Some(found) =
                    simplify::simplify_instruction(context, &crate::datalayout::DataLayout::default(), function, inst)
                    && found != Operand::Value(result)
                {
                    function.replace_value(result, found);
                    function.erase(inst).expect("replaced");
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }
        let mut counts = std::collections::BTreeMap::<&str, usize>::new();
        for (_, inst) in function.walk() {
            *counts.entry(function.instruction(inst).opcode.mnemonic()).or_default() += 1;
        }
        lines += &format!(
            "{path}\t{}\n",
            counts.iter().map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(",")
        );
    }
    std::fs::write(output, lines).expect("out");
}

#[test]
fn address_with_only_zero_indices_is_its_pointer() {
    let text = "define ptr @f(ptr %p, i16 %i) {\nb0:\n  %z = getelementptr i16, ptr %p, i16 0\n  %w = getelementptr [4 x i16], ptr %p, i16 0, i16 0\n  %v = getelementptr i16, ptr %p, i16 %i\n  ret ptr %z\n}\n";
    assert_eq!(simplified(text, "z").as_deref(), Some("%p"));
    assert_eq!(simplified(text, "w").as_deref(), Some("%p"));
    assert_eq!(simplified(text, "v"), None);
}
