//! The register counts of `WasmMachine` are the pin's `WebAssemblyTTIImpl`
//! answers for the scalar class, not a number chosen for wasm's locals:
//! `getNumberOfRegisters` = `BasicTTIImpl`'s 8 (WebAssemblyTargetTransformInfo.cpp#L31-L40,
//! BasicTTIImpl.h#L616 at d1106deb71cc).

use llrm_mir::target::Machine;
use llrm_wasm::machine::WasmMachine;

#[test]
fn the_scalar_register_count_is_the_pins() {
    assert_eq!((WasmMachine.registers(), WasmMachine.call_registers()), (8, 8));
}
