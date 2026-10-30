# QB runtime contract in the DAG path

Owner: sdag-c. Written 2026-10-24. Plan due 2026-11-03, first PR 2026-11-10.

## Why

A call of a `llrm.qb.*` routine has a contract the DAG path ignores (`B$SCMP`: stack
arguments, flags result). #4280 refuses such calls, so a qb program keeps the old selector
for every function that calls the runtime (154 of 539 functions go through the DAG path).
The DAG path cannot be the default while it refuses them.

## What the old selector does (`isel.rs` call lowering, `abi/qb.rs`)

| Part | Source | DAG path today |
|---|---|---|
| Last N arguments in registers, results in registers | `Abi::registers(name)` | convention number only |
| Remaining arguments pushed; callee pops `contract.cleanup`, caller pops `caller_cleanup` | `Abi::contract` | convention's own |
| Result is the flags, read by a compare with 0 (`flags_result`) | `Contract` | none |
| Registers destroyed | `callregs::call_clobbers(contract)` | convention's `clobbered` |
| Control (noreturn, resumes), raises error, enters user code | `Contract` | none |

## Design

`selected_in_pool` gets the `&dyn Abi` (today only a `runtime_contracts` flag). The builder
asks it once per `llrm.qb.` call for a `RuntimeCall { register_args, register_results,
stack_bytes, cleanup, flags_result, clobbers, control }` and lowers the call as an
`ISD::CALL`-like node with those fields; `emit.rs` already takes clobbers and requires/delivers
per call. Nothing is hard-coded per routine: every fact comes from `runtime.toml` through the
ABI, as in the old selector (SD deviation row per remaining gap).

## PRs (one cause each, fail-first test each)

| Date | PR | Test |
|---|---|---|
| 11-10 | ABI reaches the builder; register arguments and results of a runtime call (`B$PEI2`-style) | a runtime call with `ax` argument selects, args in `ax` |
| 11-13 | stack arguments, callee and caller cleanup | `B$SASS` pushes, pops `cleanup` |
| 11-17 | `flags_result`: the call's compare reads the flags | `B$SCMP` + `icmp` has no `test ax,ax` |
| 11-20 | clobbers from the contract; control, error and user-code effects | `si`/`di` live across `B$SCMP` are not kept |
| 11-24 | drop the #4280 refusal; qb 198 and examples 74 under `LLRM_ISEL=shadow`, DAG count reported | proof line N equals functions |

Gate: qb group under shadow passes 198/198 after each PR (DOSBox, `LLRM_ISEL_REAL=1`).
