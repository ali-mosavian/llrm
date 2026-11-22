# indirectbr / blockaddress: guard -> test (PR 1, #10353)

Each test goes red with its guard reverted.

- merge/forward (`cfg`, `local`, `emptied_into`): simplifycfg `..not_merged_into_its_predecessor`, jump threading `..not_merged..`
- SimplifyCFG hoist / two-entry phi / dedupe: `common_code_is_not_hoisted..`, `an_arm_whose_address..`, `a_jump_block_whose..`
- SCCP/IPSCCP dead block: sccp `..stays_holding_an_unreachable`
- split edge, inline viability, indirectbr folds: `addresstaken_tests`, simplifycfg `test_an_indirectbr_*`
- loop unroll header: loopunroll test; `Loop::is_safe_to_clone` (peel, jam, unswitch, versioning): llrm-mir loopinfo test
- GVN PRE (scalar + load): `addresstaken_tests`; LICM sink: `licm/tests`; interchange: `loopinterchange_tests`; dedicated exits: `loopversioning_tests`
- Rotate latch: `looprotate_tests`, red only with the `cfg` merge guard also reverted (it shadows).
