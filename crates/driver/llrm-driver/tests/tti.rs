//! Every target's mid-end entry hands the passes a cost model. With `tti() =
//! None` the passes that read it (the CRC recognizer, inline cost, loop
//! strength reduction) price by generic rules or do not run, and no output says
//! so.

use llrm_codegen::backend::cpu::tuned_optimizing;

#[test]
fn every_x86_cpu_machine_has_a_tti() {
    // the control: a machine with no cost model is what the assertion exists to
    // catch
    assert!(llrm_mir::target::Machine::tti(&llrm_mir::target::Neutral).is_none());
    let mut without = Vec::new();
    let mut checked = 0;
    for arch in llrm_driver::all() {
        for cpu in arch.cpus() {
            let profile =
                tuned_optimizing(&*arch, cpu, false, false, false, true, false, false, false, false, false, false)
                    .unwrap_or_else(|why| panic!("{}: {why}", arch.name()));
            checked += 1;
            if profile.target().tti().is_none() {
                without.push(format!("{} {cpu}", arch.name()));
            }
        }
    }
    assert!(checked > 0, "no machine was checked");
    assert!(without.is_empty(), "machines with no TTI: {without:?}");
}
