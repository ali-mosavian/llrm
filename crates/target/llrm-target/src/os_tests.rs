//! The POSIX host layer's C header is rendered from the interface; the copy in
//! the tree is the one the host runtime includes, so it must be the rendering
//! (`LLRM_BLESS=1` writes it).

use crate::os::Interface;

#[test]
fn the_posix_header_in_the_tree_is_the_interfaces_rendering() {
    let want = Interface::shipped().posix_c_header().unwrap();
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../runtime/shared/posix/llrm_os.h");
    if std::env::var_os("LLRM_BLESS").is_some() {
        std::fs::write(path, &want).unwrap();
    }
    let have = std::fs::read_to_string(path).unwrap_or_default();
    assert_eq!(have, want, "runtime/shared/posix/llrm_os.h is stale: LLRM_BLESS=1 cargo test -p llrm-target");
}

#[test]
fn the_posix_header_declares_the_time_group_and_the_core() {
    let text = Interface::shipped().posix_c_header().unwrap();
    for line in [
        "int llrm_os_open(const char *, unsigned char);",
        "int32_t llrm_os_monotonic(void);",
        "intptr_t llrm_os_local_time(unsigned char *);",
        "void llrm_os_sleep(size_t);",
    ] {
        assert!(text.contains(line), "{line} in\n{text}");
    }
}
