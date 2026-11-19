use llrm_target::Target;
use llrm_x86_m64::M64;

/// `float_stack` and `scalar_sse` parsed the whole register file at every call,
/// two per function: 469 M of the 4 725 M instructions a 600-function module
/// cost (tiny600). The file is parsed once however often it is asked.
#[test]
fn test_the_register_file_is_parsed_once_however_often_it_is_asked() {
    let (first, second) = (M64.float_stack(), M64.scalar_sse());
    let before = llrm_target::registers::parsed();
    for _ in 0..200 {
        assert_eq!(M64.float_stack(), first);
        assert_eq!(M64.scalar_sse(), second);
    }
    assert_eq!(llrm_target::registers::parsed() - before, 0);
}
