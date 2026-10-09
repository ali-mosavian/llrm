//! Each fixture is rustfmt output (`NAME.in.rs`) and what the pass must make of it (`NAME.out.rs`).

use std::io::Write;
use std::process::{Command, Stdio};

fn read(name: &str) -> String {
    std::fs::read_to_string(format!("{}/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

fn rustfmt(text: &str) -> String {
    let config = format!("{}/../../rustfmt.toml", env!("CARGO_MANIFEST_DIR"));
    let mut child = Command::new("rustfmt")
        .args(["+nightly", "--edition", "2024", "--config-path", &config])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(text.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "rustfmt failed");
    String::from_utf8(out.stdout).unwrap()
}

/// What fmt.sh does to a file.
fn pipeline(text: &str) -> String {
    let broken = rfmt_post::matches_only(&rustfmt(text)).unwrap().0;
    rfmt_post::format(&rustfmt(&broken)).unwrap().0
}

macro_rules! fixture {
    ($name:ident) => {
        mod $name {
            use super::*;

            #[test]
            fn formats_as_expected() {
                let out = pipeline(&read(concat!(stringify!($name), ".in.rs")));
                assert_eq!(out, read(concat!(stringify!($name), ".out.rs")));
            }

            /// rustfmt undoes the layout and the pass redoes it.
            #[test]
            fn pipeline_is_idempotent() {
                let expected = read(concat!(stringify!($name), ".out.rs"));
                assert_eq!(pipeline(&expected), expected);
            }
        }
    };
}

fixture!(example);
fixture!(short_chain);
fixture!(vertical);
fixture!(block_body);
fixture!(comments);
fixture!(tokens);
fixture!(joined);
fixture!(matches);
fixture!(matches_vertical);
fixture!(matches_closure);
fixture!(comment_in_operands);
fixture!(nested);
