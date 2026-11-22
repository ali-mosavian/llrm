//! `BasicTTIImplBase::getCmpSelInstrCost` multiplies a legal compare or select
//! by the registers its type splits into. The pin's M68k (`opt
//! -mtriple=m68k-unknown-linux-gnu -passes='print<cost-model>'`, every CPU):
//! i8, i16, i32, float 1; i64, double 2; i33 2; i128 4. Priced 1 for all, the
//! unroller and the speculation passes saw an `i64` select or compare as one
//! instruction (2 in the pin) and unrolled loops the pin keeps rolled by a
//! few units.

use llrm_m68k::dagisel::hooks::Cpu;
use llrm_m68k::machine::M68k;
use llrm_mir::tti::{CmpSel, Cost, CostKind, Tti, Ty};

#[test]
fn a_compare_or_select_costs_the_registers_of_its_type() {
    let cases = [
        (Ty::Int(8), 1),
        (Ty::Int(16), 1),
        (Ty::Int(32), 1),
        (Ty::Float, 1),
        (Ty::Int(33), 2),
        (Ty::Int(64), 2),
        (Ty::Double, 2),
        (Ty::Int(128), 4),
    ];
    for cpu in [Cpu::M68000, Cpu::M68020, Cpu::M68040] {
        let tti = M68k::for_cpu(cpu);
        for (ty, registers) in cases {
            for op in [CmpSel::ICmp, CmpSel::FCmp, CmpSel::Select] {
                for kind in [CostKind::SizeAndLatency, CostKind::RecipThroughput] {
                    assert_eq!(tti.cmp_sel_cost(op, ty, kind), Cost::of(registers), "{cpu:?} {op:?} {ty:?} {kind:?}");
                }
            }
        }
    }
}
