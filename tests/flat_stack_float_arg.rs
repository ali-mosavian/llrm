//! A double passed on the stack in flat code is stored through `[esp]`.

use std::process::Command;

/// Under `LLRM_VERIFY` (the gate's) `fstp qword [esp]` had no byte price: the
/// stack token is `sp`, which has no 32-bit-mode encoding, and the compile
/// panicked ("no byte price for FloatStore") on qb's gcirc.c, fin.c and fout.c;
/// this is fout.c's `fout_digits`.
#[test]
fn a_double_stored_to_the_stack_for_a_call_has_a_byte_price() {
    let scratch = tempfile::tempdir().unwrap();
    let directory = scratch.path();
    std::fs::write(
        directory.join("a.c"),
        "typedef struct { char sign; char text[24]; unsigned count; int exponent; } Decimal;\n\
extern void i8_output(double v, Decimal *d);\n\
enum { SINGLE_DIGITS = 7, DOUBLE_DIGITS = 16 };\n\
\n\
static void round_digits(Decimal *d, int *exponent, unsigned want)\n\
{\n\
    unsigned at;\n\
\n\
    if (d->count > want) {\n\
        char next = d->text[want];\n\
\n\
        *exponent += d->count - want;\n\
        d->count = want;\n\
        if (next >= '5') {\n\
            for (at = want; at; at--) {\n\
                if (d->text[at - 1] < '9') {\n\
                    d->text[at - 1]++;\n\
                    d->count = at;\n\
                    break;\n\
                }\n\
                (*exponent)++;\n\
            }\n\
            if (!at) {\n\
                d->text[0] = '1';\n\
                d->count = 1;\n\
            }\n\
        }\n\
    }\n\
    while (d->count > 1 && d->text[d->count - 1] == '0') {\n\
        d->count--;\n\
        (*exponent)++;\n\
    }\n\
}\n\
\n\
void fout_digits(double v, int is_double, Decimal *d, int *exponent)\n\
{\n\
    i8_output(v, d);\n\
    *exponent = d->exponent;\n\
    if (!(d->count == 1 && d->text[0] == '0'))\n\
        *exponent -= d->count;\n\
    round_digits(d, exponent, is_double ? DOUBLE_DIGITS : SINGLE_DIGITS);\n\
}\n",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_llrm-c"))
        .current_dir(directory)
        .env("LLRM_VERIFY", "1")
        .args(["-m32", "-Os", "-o", "a.obj", "a.c"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
}
