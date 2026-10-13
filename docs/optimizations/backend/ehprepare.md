> Generated from the `//!` header of [`crates/backend/llrm-core/src/backend/ehprepare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/backend/llrm-core/src/backend/ehprepare.rs); edit the header, not this file.
<!-- src: crates/backend/llrm-core/src/backend/ehprepare.rs sha256:d5db949ead7b692b32657ccde03a18764777d6725ec3b89dbced80fc6078f64e -->

# ehprepare

Make a QB personality's landing pad one the runtime can enter, before
instruction selection.

## How it works
The runtime keeps one handler address per module (`B$OEGA`). On an error
it builds a frame of its own over the faulting one and jumps to that
address; `B$RESN` leaves the frame again, back on the faulting one, and
continues at the first statement-table row at or after the faulting
call. So the address registered is `$QB$LANDING`, a naked function that
stores ERR in `$QB$LANDED` and calls `B$RESN`, and the function's
landing pad is the row the runtime resumes at. This is the setjmp/longjmp
shape of LLVM's SjLjEHPrepare, with the runtime as the longjmp.

```text
  call onerror(true)             call B$OEGA($QB$LANDING)
  invoke f() unwind %pad    =>   invoke f() unwind %pad
  %pad:                          %pad:
    %e = landingpad ...            %err = load i16, @$QB$LANDED
    %s = extractvalue %e, 1        (selector read from memory)
    ... uses %x (made in entry)    ... uses reload of %x's slot
```

`prepared` works on each function with the QB personality, in order:

- Checks. A function that registers a handler without the personality, a
  landing pad that resumes unwinding, a second landing pad, a call that may
  raise where ON ERROR is on and which is not an invoke to the pad, and,
  with a module-wide handler, a SUB that may raise, are refused. A forward
  analysis finds where trapping may be on (off at entry, on at a landing
  pad, set by each registration's constant argument).
- Declares the runtime routines, the landing function and `$QB$LANDED`, once
  per module, when any function has a pad. With none, each ON ERROR is
  erased because nothing can land.
- Rewrites each registration: ON ERROR GOTO becomes a call of `B$OEGA` with
  the landing's far address, or null when turned off; ON LOCAL ERROR GOTO
  calls `B$OEGP` with the landing's offset, or 0.
- Replaces the pad's selector (the landing pad's second member) with a load
  of `$QB$LANDED`.
- Demotes to the stack every phi of the pad and every value live into it
  (`demote_phis`, `demote_live`, after LLVM's DemotePHIToStack and
  DemoteRegToStack). The pad is entered with nothing the function left in a
  register: each such value is stored where it is made and reloaded where
  the pad reads it. An entry-block alloca is the frame's own address and
  stays.

## Example
A function that registers a handler, reads `A&`, and reads `%x` again in the
pad. The registration becomes a call of `B$OEGA`, the selector becomes a
load of `$QB$LANDED`, and `%x`, live into the pad, goes through the slot
`%demoted`.

Before:

```mir
define void @main() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %site = alloca i16
  call cc1000 addrspace(1) void @llrm.qb.onerror(i1 true)
  %x = load i32, ptr @"A&"
  %y = add i32 %x, 1
  ; ...
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  call cc1000 addrspace(1) void @llrm.qb.onerror(i1 false)
  %err = extractvalue { ptr, i32 } %e, 1
  %code = trunc i32 %err to i16
  store i16 %code, ptr @"CAUGHT%"
  store i32 %x, ptr @"A&"
  ; ...
```

After:

```mir
define void @main() addrspace(1) personality ptr addrspace(1) @llrm.qb.personality {
entry:
  %demoted = alloca i32
  %site = alloca i16
  call cc1000 addrspace(1) void @llrm.qb.B$OEGA(ptr addrspace(1) @$QB$LANDING)
  %x = load i32, ptr @"A&"
  store i32 %x, ptr %demoted
  %reloaded = load i32, ptr %demoted
  %y = add i32 %reloaded, 1
  ; ...
landing:
  %e = landingpad { ptr, i32 } catch ptr null
  call cc1000 addrspace(1) void @llrm.qb.B$OEGA(ptr addrspace(1) null)
  %err1 = load i16, ptr @$QB$LANDED
  %0 = sext i16 %err1 to i32
  %code = trunc i32 %0 to i16
  store i16 %code, ptr @"CAUGHT%"
  %reloaded1 = load i32, ptr %demoted
  store i32 %reloaded1, ptr @"A&"
  ; ...
```

From unit test
`the_handler_registered_is_the_landing_and_err_is_what_it_kept` in
`ehprepare_tests.rs`: its fixture `HANDLED` and the printed module.

## Runs
In `driver::optimized` (`mir ehprepare`), once per module after the MIR
pipeline and the dropping of `llvm.assume`, before `fpconvert` and
`selects`. Runs at every level. A module with no QB personality is
untouched.

## Needs
MIR with `llrm_hir::onerror` names: `ONERROR`, `ONLOCALERROR`, the
personality and `ERR`.

## See also
The phase map: `crate::backend`
