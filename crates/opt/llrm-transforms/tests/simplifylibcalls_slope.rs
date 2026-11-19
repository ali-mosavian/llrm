//! The pass is linear in the functions of a module: `strcmp(p, "x")` asks the
//! sizes of the module's globals, which are measured once for the module, not
//! once for each function. Doubling the functions (and the globals) must not
//! quadruple the time.

use std::time::Instant;

use llrm_mir::libfunc::Routine;
use llrm_mir::passes::PassManager;
use llrm_mir::target::Neutral;
use llrm_transforms::simplifylibcalls::SimplifyLibCalls;

fn module(functions: usize) -> String {
    let mut text = String::from("@a = private constant [4 x i8] c\"abc\\00\"\ndeclare i32 @strcmp(ptr, ptr)\n");
    for at in 0..functions {
        text += &format!("@g{at} = global [4 x i8] zeroinitializer\n");
        text += &format!(
            "define i32 @f{at}(ptr dereferenceable(4) %p) {{\nb0:\n  %r = call i32 @strcmp(ptr %p, ptr @a)\n  %c = icmp eq i32 %r, 0\n  %z = zext i1 %c to i32\n  ret i32 %z\n}}\n"
        );
    }
    text
}

fn seconds(functions: usize) -> f64 {
    let text = module(functions);
    (0..3)
        .map(|_| {
            let mut module = llrm_mir::parse::module(&text).expect("parses");
            let mut manager = PassManager::default();
            let memcmp = Routine { standard: "memcmp", symbol: "memcmp".into(), calling_convention: 0 };
            manager.add(SimplifyLibCalls::with_routines(vec![memcmp]));
            let start = Instant::now();
            manager.run_module(&mut module, std::rc::Rc::new(Neutral)).expect("runs");
            assert!(llrm_mir::print::module(&module).contains("@memcmp(ptr %p, ptr @a, i64 4)"));
            start.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn test_doubling_the_functions_does_not_quadruple_the_time() {
    let (small, large) = (seconds(600), seconds(1200));
    eprintln!("600: {small:.4}s, 1200: {large:.4}s, ratio {:.2}", large / small);
    assert!(large < 3.0 * small, "600 functions {small}s, 1200 functions {large}s");
}
