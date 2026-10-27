> Generated from the `//!` header of [`crates/opt/llrm-transforms/src/settled.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/opt/llrm-transforms/src/settled.rs); edit the header, not this file.
<!-- src: crates/opt/llrm-transforms/src/settled.rs sha256:6637827e724ecf3a306fdc3bff5a7801ac07a402185416cec433ba8a40fe14dc -->

# settled

What each pass found nothing to do on, kept from one run of the pipeline
over a body to the next: a body goes through its pipeline again only where
something a pass read of it has changed.

## How it works
A pass that changed nothing is noted as it runs: the parts of the module's
facts it read (`llrm_mir::passes::read`, noted by the analyses it asks for
and by what it asks of the proxy). When a run ends, each such pass that no
edit has reached since (`FunctionPass::reads`, `llrm_mir::footprint`) is
recorded for the body's history and its name: the point in the change log,
and a number for each part of what it could read outside the body (`slice`).
Asked to run again, the pass is skipped when the record holds: the body's
own attributes and every part the pass read are the same, and the edits
logged since reach nothing it reads. A pass that does change the body drops
its record.

The body's own are its attributes and the module's metadata length. The
parts are, for each global the body names (a callee, a variable it takes the
address of, one a call's `!callees` lists): its declaration; what the callee
does to memory and the registers its call keeps; its summary; and, of the
module, what the globals analysis holds of those globals and what calling
back into the module does. A callee is taken by what is known of it, not by
its id: a call sent to a copy that is known to do the same is the same fact.

An edit that a stage vouches for (`Settled::vouch`) is left out of what the
edits reach: a call sent to a copy of its callee whose attributes are the
same, an argument dropped that nothing else read. The facts' numbers check
the vouching: a copy that does something else is another number.

```text
  first run     fold gvn decide ...  idle   → records at mark 40, facts F
  stage edits   call g → call g.constprop.1   (vouched, g and its copy: F)
  second run    fold gvn decide ...  F same, nothing since reaches them → skipped
  a callee's memory effects change
  third run     F differs → each pass runs
```

`LLRM_CHECK_SETTLED=1` runs every pass that would be skipped and fails if
it changes the body.

## Example
A body that calls `@g` is run through the pipeline, and a stage then sends
its call to `@g.copy`, a copy with the same attributes:

```mir
define i16 @f(i16 %x) {
b0:
  %a = call i16 @g(i16 %x)
  ret i16 %a
}
```

The second run starts no pass: the facts of the callee are the same and the
stage vouched for the edit. Where `@g.copy` writes memory that `@g` does
not, the facts differ and every pass runs again.

From unit test
`test_a_body_run_again_after_a_call_is_sent_to_an_equal_copy_starts_no_pass`
in `pipeline_tests.rs`.

## Provides
`Settled` (the records, held by the module's `Context`) and `slice`.

## Needs
[`llrm_mir::footprint`](https://github.com/ali-mosavian/llrm/blob/main/crates/ir/llrm-mir/src/footprint.rs), [`llrm_analysis::globalsaa`](../analyses/globalsaa.md), and the module
analyses of the proxy a pass reads (`Outer`).

## See also
LLVM's preserved analyses and gcc's `TODO_*` flags, which say the same of
what a pass needs to look at again; [`crate::pipeline`](pipeline.md)'s per-run settled
test, which this extends across runs.
