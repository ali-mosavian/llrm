//! Port of `qbopt/optimize/transform.py`: MIR transforms, a body in, an
//! optimised body out.

// A root register at the width an operand reads it. ir.ROOT maps the narrow
// name to the wide one; this is the way back, and only for the general
// registers -- a segment register has no narrower form and is never a
// provider here.

#[cfg(any(test, feature = "testing"))]
thread_local! {
    /// Half-liveness fixed points solved, for the tests that pin reuse to Python's.
    pub static HALVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
