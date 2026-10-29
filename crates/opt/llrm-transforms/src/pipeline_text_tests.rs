use std::rc::Rc;

use llrm_mir::passes::PassManager;

use super::*;

fn pass(name: &str) -> Element {
    Element::Pass { name: name.to_owned(), params: None }
}

fn run(
    pipeline: &str,
    text: &str,
) -> Result<String, Unsupported> {
    let mut module = llrm_mir::parse::module(text).unwrap();
    let mut manager = PassManager::default();
    build(&parse(pipeline).unwrap(), &mut manager)?;
    manager.run_module(&mut module, Rc::new(llrm_mir::target::Neutral)).unwrap();
    Ok(llrm_mir::print::module(&module))
}

const DEAD: &str = "define i32 @f(i32 %x) {\nentry:\n  %dead = mul i32 %x, 3\n  ret i32 %x\n}\n";

#[test]
fn passes_and_nested_pipelines_keep_their_order_and_parameters() {
    assert_eq!(parse("loop-reduce,indvars").unwrap(), [pass("loop-reduce"), pass("indvars")]);
    assert_eq!(
        parse("function(loop(indvars<no-lftr>,require<scalar-evolution>),dce)").unwrap(),
        [Element::Nested {
            name: "function".to_owned(),
            inner: vec![
                Element::Nested {
                    name: "loop".to_owned(),
                    inner: vec![
                        Element::Pass { name: "indvars".to_owned(), params: Some("no-lftr".to_owned()) },
                        Element::Pass { name: "require".to_owned(), params: Some("scalar-evolution".to_owned()) },
                    ],
                },
                pass("dce"),
            ],
        }]
    );
}

#[test]
fn malformed_text_is_an_error_not_a_shorter_pipeline() {
    for bad in ["loop(indvars", "indvars)", "a,,b", "indvars<no-lftr", ",dce"] {
        assert!(parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn a_pass_this_crate_lacks_is_named_unsupported_and_the_rest_do_not_run() {
    for (text, named) in [
        ("loop-term-fold", "loop-term-fold"),
        ("function(dce,default<O2>)", "default<O2>"),
        ("cgscc(inline)", "pass adaptor cgscc"),
        ("indvars<widen>", "indvars<widen>"),
    ] {
        let mut manager = PassManager::default();
        assert_eq!(build(&parse(text).unwrap(), &mut manager), Err(Unsupported(named.to_owned())), "{text}");
    }
}

#[test]
fn dce_in_a_pipeline_removes_what_nothing_reads() {
    let out = run("function(dce)", DEAD).unwrap();
    assert!(!out.contains("%dead"), "{out}");
    let out = run("require<domtree>,verify", DEAD).unwrap();
    assert!(out.contains("%dead"), "analyses and verify change nothing:\n{out}");
}

#[test]
fn legacy_flags_name_the_same_passes() {
    assert_eq!(legacy("loop-reduce"), Some("loop-reduce"));
    assert_eq!(legacy("gvn"), None);
}

const EXIT_USE: &str = "define i32 @f(i32 %n) {\nentry:\n  br label %loop\nloop:\n  %i = phi i32 [ 0, %entry ], [ %next, %loop ]\n  %next = add i32 %i, 1\n  %c = icmp ult i32 %next, %n\n  br i1 %c, label %loop, label %exit\nexit:\n  ret i32 %next\n}\n";

/// `-passes=loop-reduce` on the pin runs LoopSimplify and LCSSA first (the loop
/// adaptor's canonicalisation), so the pin's output keeps a phi for the value
/// read after the loop; without it a test's `phi` CHECKs saw none.
#[test]
fn a_run_of_loop_passes_is_canonicalised_into_lcssa_form_first() {
    let phis = |out: &str| out.matches(" = phi ").count();
    assert_eq!(phis(&run("loop(loop-instsimplify)", EXIT_USE).unwrap()), 2);
    assert_eq!(phis(&run("function(dce)", EXIT_USE).unwrap()), 1);
}
