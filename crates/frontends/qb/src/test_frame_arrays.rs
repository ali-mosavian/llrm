//! A constant-bound array in a procedure is the frame's, as C's is.

use super::driver as qb_driver;
use super::test_hir::{optimized_mir, written};

const SIEVE: &str = "DEFINT A-Z
DECLARE FUNCTION Sieve& (limit AS INTEGER)
PRINT LTRIM$(STR$(Sieve&(1024)))

FUNCTION Sieve& (limit AS INTEGER)
    DIM composite(0 TO 1023) AS INTEGER
    DIM i AS INTEGER, multiple AS INTEGER, count AS INTEGER
    FOR i = 0 TO limit - 1
        composite(i) = 0
    NEXT
    FOR i = 2 TO limit - 1
        IF composite(i) = 0 THEN
            count = count + 1
            multiple = i * i
            DO WHILE multiple < limit
                composite(multiple) = 1
                multiple = multiple + i
            LOOP
        END IF
    NEXT
    Sieve& = count
END FUNCTION
";

fn optimized(source: &str) -> String {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "sieve.bas", source.as_bytes());
    let frontend = qb_driver::Frontend {
        whole_program: true,
        frame_array_bytes: 2048,
        ..qb_driver::Frontend::new("qb45", "llrm")
    };
    let program = qb_driver::parsed(&path, &frontend, None).expect("compiles");
    optimized_mir(&program)
}

/// bench/sieve ran B$DDIM and B$ERAS on every call and read the array through a
/// far pointer (bq16 43,002 clocks against 31,801 with the array near, C
/// 22,757). The array is 2 KB with constant bounds, never resized.
#[test]
fn a_constant_bound_local_array_is_not_allocated_by_the_runtime() {
    let mir = optimized(SIEVE);
    assert!(!mir.contains("B$DDIM"), "{mir}");
    assert!(!mir.contains("B$ERAS"), "{mir}");
}

fn built(source: &str) -> String {
    optimized(source)
}

const SORT: &str = "DEFINT A-Z
DECLARE SUB Fill (a() AS INTEGER, n AS INTEGER)
DECLARE FUNCTION Total& (n AS INTEGER)
PRINT LTRIM$(STR$(Total&(8)))

SUB Fill (a() AS INTEGER, n AS INTEGER)
    DIM i AS INTEGER
    FOR i = 0 TO n - 1
        a(i) = i * 3
    NEXT
END SUB

FUNCTION Total& (n AS INTEGER)
    DIM values(0 TO 15) AS INTEGER
    DIM i AS INTEGER, sum AS LONG
    Fill values(), n
    FOR i = 0 TO n - 1
        sum = sum + values(i)
    NEXT
    Total& = sum
END FUNCTION
";

/// A callee of a frame array read it through a far pointer built from the
/// descriptor's selector and offset (`es:` in every access: bq16 quicksort, 4
/// more instructions per call than the offset alone). With no array in the far
/// heap the callee's access is near.
#[test]
fn a_callee_of_a_frame_array_reaches_its_elements_near() {
    let mir = built(SORT);
    assert!(!mir.contains("B$DDIM"), "{mir}");
    assert!(!mir.contains("ptr addrspace(1)") && !mir.contains("ptr addrspace(2)"), "{mir}");
}

/// A REDIM in the program may resize any array handed to it, so the array
/// stays the runtime's.
#[test]
fn an_array_that_is_redimmed_stays_the_runtimes() {
    let resized = SORT.replace("    DIM i AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)", "    DIM i AS INTEGER\n    REDIM a(n + 1) AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)");
    assert!(built(&resized).contains("B$DDIM"), "a formal that is REDIMmed was framed");
}

/// A DIM that is not at the top of the procedure may run more than once, which
/// BC refuses (error 10); the frame form would not.
#[test]
fn a_dim_inside_a_loop_stays_the_runtimes() {
    let nested = SORT.replace("    DIM values(0 TO 15) AS INTEGER\n", "    DIM k AS INTEGER\n    FOR k = 0 TO 1\n        DIM values(0 TO 15) AS INTEGER\n    NEXT\n");
    assert!(built(&nested).contains("B$DDIM"), "a nested DIM was framed");
}

/// The array is the stack's, so it has a budget: 2 KB.
#[test]
fn an_array_over_the_budget_stays_the_runtimes() {
    let big = SORT.replace("0 TO 15", "0 TO 1999");
    assert!(built(&big).contains("B$DDIM"), "a 4000-byte array was framed");
}

const TOUCH: &str = "DEFINT A-Z
DECLARE SUB Touch (x AS INTEGER, a() AS INTEGER)
DECLARE FUNCTION Probe% ()
PRINT Probe%

SUB Touch (x AS INTEGER, a() AS INTEGER)
    a(1) = 99
END SUB

FUNCTION Probe%
    DIM t(0 TO 9) AS INTEGER
    t(1) = 5
    Touch t(1), t()
    Probe% = t(1)
END FUNCTION
";

/// BC copies an element passed by reference out of its far array into a slot and
/// back after the call, so `Touch t(1), t()` leaves 5 where the callee stored 99
/// (real BC 4.5 prints 5). With the array in the frame the element was passed
/// by address and the program printed 99.
#[test]
fn an_element_of_a_frame_array_is_still_copied_in_and_out() {
    let mir = built(TOUCH);
    assert!(!mir.contains("B$DDIM"), "{mir}");
    assert!(mir.contains("B$PEI2(i16 5)"), "{mir}");
}
