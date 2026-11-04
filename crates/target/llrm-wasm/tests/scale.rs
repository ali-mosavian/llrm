//! The passes are linear in the size of the function: doubling it must not
//! quadruple the time. A quadratic scan per register (`use_empty`, `getVRegDef`
//! over the whole function) did.

use std::time::Instant;

use llrm_wasm::Width;
use llrm_wasm::dag::Options;
use llrm_wasm::driver::assembly;

/// `n` diamonds in a row, each with a phi.
fn chain(n: usize) -> String {
    let mut text = String::from("define i32 @f(i32 %a) {\nentry:\n  br label %d0\n");
    for i in 0..n {
        text += &format!(
            "d{i}:\n  %c{i} = icmp slt i32 %a, {i}\n  br i1 %c{i}, label %t{i}, label %e{i}\n\
             t{i}:\n  %x{i} = add i32 %a, {i}\n  br label %e{i}\n\
             e{i}:\n  %j{i} = phi i32 [ %x{i}, %t{i} ], [ %a, %d{i} ]\n  br label %d{}\n",
            i + 1
        );
    }
    text + &format!("d{n}:\n  ret i32 %a\n}}\n")
}

fn best_of_three(n: usize) -> f64 {
    let text = chain(n);
    (0..3)
        .map(|_| {
            let start = Instant::now();
            assembly(&text, Width::W32, Options::default()).expect("compiles");
            start.elapsed().as_secs_f64()
        })
        .fold(f64::MAX, f64::min)
}

#[test]
fn doubling_the_function_does_not_quadruple_the_time() {
    let (small, large) = (best_of_three(500), best_of_three(2000));
    // 4x the size: linear is 4x, quadratic 16x.
    assert!(large / small < 8.0, "500 diamonds {small:.3}s, 2000 diamonds {large:.3}s: x{:.1}", large / small);
}
