//! A word past the width stays whole:
//! https://github.com/ali-mosavian/llrm/issues/794/with/a/path/that/runs/on/and/on/past/column/eighty
//!
//! ```text
//! llrm SOURCE [--entry ENTRY] [--dump DIR] [--procedure-segments] [--used-by OBJ]... [--unchecked-bounds]
//! ```
//!
//! | name | a table row is not prose, whatever its width, so it stays as it is written, cells and all |
//! | ---- | ---- |

fn f() {}
