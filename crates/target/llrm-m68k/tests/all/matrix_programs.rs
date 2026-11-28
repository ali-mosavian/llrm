//! `matrix` ran all 22 programs at four CPUs and four compilers under qemu, or
//! none: re-measuring a few cells cost the whole run. `MATRIX_PROGS` names the
//! programs; a name no program has must stop the run, not print an empty
//! matrix.

use llrm_m68k::kernels::{keep_named, library_table, table};

#[test]
fn a_program_filter_keeps_exactly_the_named_programs() {
    let kept = keep_named(table(), |row| row.0, "arrmax,strlen").expect("both exist");
    let mut names: Vec<&str> = kept.iter().map(|row| row.0).collect();
    names.sort();
    assert_eq!(names, ["arrmax", "strlen"]);
    let library = keep_named(library_table(), |row| row.0, "hex,partition").expect("both exist");
    assert_eq!(library.len(), 2);
}

#[test]
fn an_unknown_or_empty_program_filter_is_an_error() {
    let why = keep_named(table(), |row| row.0, "arrmax,nosuch").err().expect("an unknown name is an error");
    assert!(why.contains("nosuch"), "{why}");
    assert!(keep_named(table(), |row| row.0, "").is_err());
}
