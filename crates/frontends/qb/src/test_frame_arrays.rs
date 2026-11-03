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
    let frontend = qb_driver::Frontend::new("qb45", "qb45");
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
