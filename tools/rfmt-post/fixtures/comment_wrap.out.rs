fn f(
    what: &What,
    body: &Body,
) {
    match what {
        Some(what)
            if what.op == Operation::Call
                && what.indirect
                // A far pointer packs selector and offset in four bytes of a
                // 16-bit segment; in a 32-bit one a dword is a near pointer.
                && body.bits == 16 => {}
        _ => {}
    }
}

static TABLE: [(&str, i64); 2] = [
    // Path descriptor, channel, record length -1 and mode; BCOM45 dkopen.asm
    // B$OPEN at 0224 returns with RETF 8 at 0252, and a word more to pass the
    // width.
    ("B$OPEN", 8),
    ("B$SLEP", 4),
];

thread_local! {
    /// Block transfers worked out, for a test that a block whose input is as it
    /// was is not worked again.
    pub static WORKED: Cell<usize> = const { Cell::new(0) };
}

fn g() {
    let roots = |named: &[u32]| named.len(); // short
    call(
        // The target now comes from the cell; what the call still reads (its
        // register arguments) stays read. A second sentence follows, which
        // carries on past the width as well, so that it breaks.
        a, b,
    );
}
