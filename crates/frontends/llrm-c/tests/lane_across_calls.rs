//! A call destroys the high halves of ESI and EDI at 16 bits: a value read
//! only as a word still lives across it in SI or DI.

/// The listing of `source` for 16-bit x86 at -Os.
fn listing(source: &str) -> String {
    let dir = tempfile::tempdir().expect("a directory");
    let (input, output) = (dir.path().join("in.c"), dir.path().join("out.s"));
    std::fs::write(&input, source).expect("the source");
    let argv: Vec<String> = ["-m16", "-march=i386", "-Os", "-S", "-o"]
        .iter()
        .map(|one| one.to_string())
        .chain([output.display().to_string(), input.display().to_string()])
        .collect();
    assert_eq!(llrm_c::compile::main(&argv), 0);
    std::fs::read_to_string(output).expect("the listing")
}

/// Whether an instruction names `register` (`.model medium` holds "edi").
fn names(
    text: &str,
    register: &str,
) -> bool {
    text.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| word == register)
}

/// The word of a far result read after a call was a dword in the frame
/// (`mov dword ptr [bp-4], eax` and back): its definition is 32 bits wide, so
/// SI was thought lost across the call.
#[test]
fn a_word_read_after_a_call_stays_in_a_register() {
    let text = listing(
        "extern long mk(void); extern void g(void);\nshort f(void) { long a = mk(); short s = (short)a; g(); return s + 1; }\n",
    );
    assert!(!text.contains("[bp"), "{text}");
}

/// The offset of a far pointer was reloaded as a dword before each compare
/// (`mov ebx, dword ptr [bp-8]`, four times in a row after one call).
#[test]
fn a_far_offset_is_not_reloaded_between_compares() {
    let text = listing(
        "typedef struct { long magic; short a, b, c, d; } M;\n\
         extern M far *mk(void); extern void copy(M far *d, M far *s, short n); extern void fail(const char *s);\n\
         void f(void) { M far *m = mk(); copy(m, m, 168);\n\
         if (m->magic != 827347793L || m->a != 1 || m->b != 3 || m->c != 9 || m->d != 256) fail(\"bad\"); }\n",
    );
    assert!(!text.contains("mov ebx, dword ptr [bp"), "{text}");
}

/// A value read as a dword after the call still loses its high half there: it
/// is not kept in SI or DI.
#[test]
fn a_dword_read_after_a_call_is_spilled_whole() {
    let text =
        listing("extern long mk(void); extern void g(void);\nlong f(void) { long a = mk(); g(); return a + 1; }\n");
    assert!(text.contains("dword ptr [bp"), "{text}");
    assert!(!names(&text, "esi") && !names(&text, "edi"), "{text}");
}

/// The wide read is on the copy, not on the value read as a word after the
/// call: the copy's value is still whole across the call.
#[test]
fn a_dword_read_through_a_copy_is_spilled_whole() {
    let text = listing(
        "extern long mk(void); extern void g(void); extern void h(long);\n\
         short f(void) { long a = mk(); long b = a; g(); h(b); return (short)a; }\n",
    );
    assert!(!names(&text, "esi") && !names(&text, "edi"), "{text}");
}

/// The dword read sits above the call in the loop body, so the next trip reads
/// the value the call went over.
#[test]
fn a_dword_read_earlier_in_the_loop_body_is_spilled_whole() {
    let text = listing(
        "extern long mk(void); extern void g(void); extern void h(long);\n\
         void f(short n) { long a = mk(); short i; for (i = 0; i < n; i++) { h(a); g(); } }\n",
    );
    assert!(!names(&text, "esi") && !names(&text, "edi"), "{text}");
}

/// The region split saw SI and EDI destroyed by every call, so the offset of a
/// far pointer used past a call in the compares after it was spilled whole and
/// reloaded into BX before each compare (far2: 19 frame accesses, 4 `mov ebx`).
#[test]
fn a_far_offset_stays_in_a_register_through_compares_and_a_later_call() {
    let text = listing(
        "typedef struct { long magic; short a, b, c, d; } M;\n\
         typedef struct { M far *p; short z; } W;\n\
         extern M far *mk(void); extern M far *row(void); extern void fail(const char *s);\n\
         extern void copy(M far *d, M far *s, short n);\n\
         void f(W *w) { M far *m = mk(); M far *r; if (!m) fail(\"a\"); r = row(); if (!r) fail(\"b\");\n\
         copy(m, r, 168);\n\
         if (m->magic != 827347793L || m->a != 1 || m->b != 3 || m->c != 9 || m->d != 256) fail(\"bad\");\n\
         w->p = m; }\n",
    );
    assert!(!text.contains("mov ebx"), "{text}");
}
