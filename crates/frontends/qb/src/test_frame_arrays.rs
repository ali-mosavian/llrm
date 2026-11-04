//! A constant-bound array in a procedure is the frame's, as C's is.

use super::driver as qb_driver;
use super::test_hir::{optimized_mir, written};

/// The 16-bit target's: six integer registers, three of them address registers,
/// three segment registers, one kept back.
const M16: crate::semantic::Room = crate::semantic::Room { registers: 6, addresses: 3, segments: 3, reserve: 1 };

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
        frame_array_bytes: 8192,
        room: M16,
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
    let resized = SORT.replace(
        "    DIM i AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)",
        "    DIM i AS INTEGER\n    REDIM a(n + 1) AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)",
    );
    assert!(built(&resized).contains("B$DDIM"), "a formal that is REDIMmed was framed");
}

/// A DIM that is not at the top of the procedure may run more than once, which
/// BC refuses (error 10); the frame form would not.
#[test]
fn a_dim_inside_a_loop_stays_the_runtimes() {
    let nested = SORT.replace(
        "    DIM values(0 TO 15) AS INTEGER\n",
        "    DIM k AS INTEGER\n    FOR k = 0 TO 1\n        DIM values(0 TO 15) AS INTEGER\n    NEXT\n",
    );
    assert!(built(&nested).contains("B$DDIM"), "a nested DIM was framed");
}

/// The array is the stack's, so it has a budget: 8 KB.
#[test]
fn an_array_over_the_budget_stays_the_runtimes() {
    let big = SORT.replace("0 TO 15", "0 TO 4999");
    assert!(built(&big).contains("B$DDIM"), "a 10000-byte array was framed");
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

/// An element of a frame array is passed by its address, so `Touch t(1), t()`
/// leaves 99. (BC's far-array copy-in/out would leave 5; that is its shape, not
/// the program's meaning.)
#[test]
fn an_element_of_a_frame_array_is_passed_by_address() {
    let mir = built(TOUCH);
    assert!(!mir.contains("B$DDIM"), "{mir}");
    assert!(mir.contains("B$PEI2(i16 99)"), "{mir}");
}

/// Each call of a recursive procedure would take another array off a stack
/// that has room for few, so its arrays stay the heap's.
#[test]
fn a_recursive_procedures_array_stays_the_runtimes() {
    let source = "DEFINT A-Z
DECLARE SUB Walk (n AS INTEGER)
Walk 9

SUB Walk (n AS INTEGER)
    DIM buf(0 TO 15) AS INTEGER
    buf(0) = n
    IF n > 0 THEN Walk n - 1
    PRINT buf(0)
END SUB
";
    assert!(built(source).contains("B$DDIM"), "a recursive procedure's array was framed");
}

/// The frame arrays of the procedures together are held to what the stack has
/// room for: three of 2000 bytes do not fit in 4096, so the last stays the
/// heap's.
#[test]
fn frame_arrays_together_stay_within_the_stack() {
    let source = "DEFINT A-Z
DECLARE SUB A ()
DECLARE SUB B ()
DECLARE SUB C ()
A
B
C

SUB A
    DIM x(0 TO 2499) AS INTEGER
    x(1) = 1: PRINT x(1)
END SUB

SUB B
    DIM y(0 TO 2499) AS INTEGER
    y(1) = 2: PRINT y(1)
END SUB

SUB C
    DIM z(0 TO 2499) AS INTEGER
    z(1) = 3: PRINT z(1)
END SUB
";
    assert_eq!(built(source).matches("call cc 1000 addrspace(1) void @llrm.qb.B$DDIM").count(), 2, "{}", built(source));
}

fn invariant_facts(source: &str) -> usize {
    let directory = tempfile::tempdir().expect("creates a directory");
    let path = written(&directory, "sieve.bas", source.as_bytes());
    let frontend = qb_driver::Frontend {
        whole_program: true,
        frame_array_bytes: 8192,
        ..qb_driver::Frontend::new("qb45", "llrm")
    };
    let program = qb_driver::parsed(&path, &frontend, None).expect("compiles");
    program
        .modules
        .iter()
        .flat_map(|module| &module.facts)
        .filter(|stated| matches!(stated.fact, llrm_hir::facts::Fact::Invariant))
        .count()
}

/// A frame array is DIM'd in its procedure, not in `__main`'s entry block, so
/// no descriptor load was invariant: bq16 quicksort's `Sort` reloaded the
/// origin in its loop and kept the descriptor pointer, 6 more clocks per
/// element than C's array pointer, because ArgPromotion could not take it.
#[test]
fn a_frame_arrays_descriptor_words_are_invariant_loads() {
    assert!(invariant_facts(SORT) > 0);
}

/// The control: a REDIM can rewrite the descriptor, so nothing is invariant.
#[test]
fn a_redimmed_arrays_descriptor_words_are_not_invariant_loads() {
    let resized = SORT.replace(
        "    DIM i AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)",
        "    DIM i AS INTEGER\n    REDIM a(n + 1) AS INTEGER\n    FOR i = 0 TO n - 1\n        a(i)",
    );
    assert_eq!(invariant_facts(&resized), 0);
}

/// bq16 quicksort ran 288K clocks with its array in the frame against 264K
/// with the runtime's: `Sort` holds the array's base beside the six values the
/// loop keeps, and spills. The segment registers hold a far array's base for
/// nothing, so an array handed to a loop like that stays the runtime's.
#[test]
fn an_array_parameter_indexed_twice_in_a_hot_loop_stays_the_runtimes() {
    let source = "DEFINT A-Z
DECLARE SUB Sort (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)
DECLARE FUNCTION Drive& ()
PRINT LTRIM$(STR$(Drive&))

SUB Sort (a() AS INTEGER, lo AS INTEGER, hi AS INTEGER)
    DIM pivot AS INTEGER, i AS INTEGER, j AS INTEGER, t AS INTEGER
    IF lo >= hi THEN EXIT SUB
    pivot = a(hi)
    i = lo
    FOR j = lo TO hi - 1
        IF a(j) < pivot THEN
            t = a(i): a(i) = a(j): a(j) = t
            i = i + 1
        END IF
    NEXT
    t = a(i): a(i) = a(hi): a(hi) = t
    Sort a(), lo, i - 1
    Sort a(), i + 1, hi
END SUB

FUNCTION Drive&
    DIM values(0 TO 63) AS INTEGER
    DIM k AS INTEGER
    FOR k = 0 TO 63
        values(k) = (k * 37) AND 63
    NEXT
    Sort values(), 0, 63
    Drive& = values(5)
END FUNCTION
";
    assert!(built(source).contains("B$DDIM"), "{}", built(source));
}

/// The control: a loop with registers to spare keeps the array in the frame,
/// as bq16 prefix does (4% fewer clocks, 20% fewer bytes).
#[test]
fn an_array_parameter_in_a_loop_with_registers_to_spare_stays_in_the_frame() {
    let source = "DEFINT A-Z
DECLARE SUB Prefix (a() AS INTEGER, n AS INTEGER)
DECLARE FUNCTION Drive& ()
PRINT LTRIM$(STR$(Drive&))

SUB Prefix (a() AS INTEGER, n AS INTEGER)
    DIM i AS INTEGER
    FOR i = 1 TO n - 1
        a(i) = (a(i) + a(i - 1)) AND 4095
    NEXT
END SUB

FUNCTION Drive&
    DIM v(0 TO 31) AS INTEGER
    DIM k AS INTEGER
    FOR k = 0 TO 31
        v(k) = k
    NEXT
    Prefix v(), 32
    Drive& = v(5)
END FUNCTION
";
    assert!(!built(source).contains("B$DDIM"), "{}", built(source));
}

/// bq16 grep ran 51.8M clocks against 44.4M: the scan loop sums two values
/// read from arrays into a subscript, which no instruction forms in an address
/// of a frame array (`[bp+si+bx]` is not one), so it takes the register the
/// loop did not have. The arrays of that loop stay the runtime's, together.
#[test]
fn arrays_read_into_each_others_subscripts_in_a_hot_loop_stay_the_runtimes() {
    let source = "DEFINT A-Z
DECLARE FUNCTION Drive& ()
PRINT LTRIM$(STR$(Drive&))

FUNCTION Drive&
    DIM nxt(0 TO 255) AS INTEGER, classof(0 TO 31) AS INTEGER, hit(0 TO 7) AS INTEGER
    DIM s AS INTEGER, c AS INTEGER, k AS INTEGER, n AS INTEGER
    DIM sum AS LONG
    FOR k = 0 TO 7
        hit(k) = k AND 1
    NEXT
    s = 0
    FOR n = 1 TO 30
        FOR k = 0 TO 99
            c = (k * 7) AND 31
            s = nxt(s * 32 + classof(c)) AND 7
            IF hit(s) <> 0 THEN sum = sum + 1
        NEXT
    NEXT
    Drive& = sum
END FUNCTION
";
    assert!(built(source).contains("B$DDIM"), "{}", built(source));
}

/// bq16 bubble sort at -Os ran 5% more clocks with its array in the frame:
/// the inner loop's counter, the outer loop's counter, the array's base and the
/// two element addresses and values want seven of six registers, and the outer
/// counter goes to memory. The outer loop's counter is live through the
/// inner loop without being named in it. The model also reaches this
/// answer through the array parameter's descriptor words counted as live
/// scalars (a known approximation, see `live_at`): without that count the
/// loop is judged to fit and this test fails.
#[test]
fn an_array_parameter_in_a_loop_nest_stays_the_runtimes() {
    let source = "DEFINT A-Z
DECLARE SUB Bubble (a() AS INTEGER, n AS INTEGER)
DECLARE FUNCTION Drive& ()
PRINT LTRIM$(STR$(Drive&))

SUB Bubble (a() AS INTEGER, n AS INTEGER)
    DIM i AS INTEGER, j AS INTEGER, t AS INTEGER
    FOR i = n - 1 TO 1 STEP -1
        FOR j = 0 TO i - 1
            IF a(j) > a(j + 1) THEN
                t = a(j): a(j) = a(j + 1): a(j + 1) = t
            END IF
        NEXT
    NEXT
END SUB

FUNCTION Drive&
    DIM v(0 TO 31) AS INTEGER
    DIM k AS INTEGER
    FOR k = 0 TO 31
        v(k) = (k * 37) AND 31
    NEXT
    Bubble v(), 32
    Drive& = v(5)
END FUNCTION
";
    assert!(built(source).contains("B$DDIM"), "{}", built(source));
}
