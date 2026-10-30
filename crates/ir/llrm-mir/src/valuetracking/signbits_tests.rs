//! `sb` against the lines LLVM printed (`tools/llvmport/vtdump.sh sb`) for
//! `tests/vt/signbits/ops.ll`.

use super::AssumptionCache;
use super::Query;
use crate::datalayout::DataLayout;
use crate::domtree::DomTree;
use crate::module::{GlobalKind, Operand};
use crate::parse;

fn dump(text: &str) -> String {
    let module = parse::module(text).expect("parses");
    let layout = DataLayout::default();
    let mut out = String::new();
    for global in &module.globals {
        let GlobalKind::Function(function) = &global.kind else { continue };
        if function.is_declaration() {
            continue;
        }
        let dt = DomTree::of(function);
        let assumptions = AssumptionCache::new(&module, function);
        let entry_first = function.block(function.entry().expect("an entry")).first();
        let mut values: Vec<_> = function.parameters().iter().map(|p| (Operand::Value(*p), entry_first)).collect();
        for (_, inst) in function.walk() {
            if let Some(result) = function.instruction(inst).result {
                values.push((Operand::Value(result), Some(inst)));
            }
        }
        let mut n = 0;
        for (value, cxt) in values {
            let mut q = Query::new(&module, function, &layout).with_cxt(cxt);
            q.dt = Some(&dt);
            q.assumptions = Some(&assumptions);
            if q.scalar_bits(value) == 0 {
                continue;
            }
            let name = global.name.owned().unwrap_or_default();
            out.push_str(&format!(
                "{name} {n}{}
",
                super::signbits::column(&q, value)
            ));
            n += 1;
        }
    }
    out
}

/// A sign-bit count that is not LLVM's changes what a selector may fold.
#[test]
fn test_sign_bits_match_llvm_on_ops() {
    let expected = r"ext 0 sb=1
ext 1 sb=1
ext 2 sb=1
ext 3 sb=25
ext 4 sb=17
ext 5 sb=8
ext 6 sb=22
ext 7 sb=1
ext 8 sb=17
ext 9 sb=8
ext 10 sb=8
ext 11 sb=16
ext 12 sb=1
ext 13 sb=16
ext 14 sb=9
ext 15 sb=17
ext 16 sb=21
ext 17 sb=1
ext 18 sb=28
ext 19 sb=25
ext 20 sb=9
ext 21 sb=1
ext 22 sb=1
sel 0 sb=1
sel 1 sb=1
sel 2 sb=1
sel 3 sb=1
sel 4 sb=25
sel 5 sb=17
sel 6 sb=17
sel 7 sb=1
sel 8 sb=1
sel 9 sb=16
sel 10 sb=1
sel 11 sb=1
sel 12 sb=24
sel 13 sb=1
sel 14 sb=24
loop 0 sb=1
loop 1 sb=1
loop 2 sb=1
loop 3 sb=1
loop 4 sb=25
loop 5 sb=17
loop 6 sb=17
loop 7 sb=1
";
    assert_eq!(dump(include_str!("../../tests/vt/signbits/ops.ll")), expected);
}
