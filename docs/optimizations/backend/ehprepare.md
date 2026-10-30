> Generated from the `//!` header of [`crates/codegen/llrm-codegen/src/backend/ehprepare.rs`](https://github.com/ali-mosavian/llrm/blob/main/crates/codegen/llrm-codegen/src/backend/ehprepare.rs); edit the header, not this file.
<!-- src: crates/codegen/llrm-codegen/src/backend/ehprepare.rs sha256:14010e2c1316ef43a6dfe8f8594968c39433c1b5de881b1550b1512fdfbb764a -->

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

The frontend passes each of a handled function's variables to `FRAME`
(`llrm.qb.frame`) in its entry, so every pass sees it captured and keeps its
loads and stores: the handler reads the variables whenever a call raises, as
an SEH funclet reads its parent's frame (`llvm.localescape`). A statement's
own argument boxes (`$arg`) are not passed, nor is a variable
nothing writes (it holds the zero it was made with) or one dead at every
statement RESUME continues at, a handler's first block, and the statement
after each that may raise (`resumable_groups`): qb/bounds.bas -261 B at -Os.
Without it GVN forwarded the variables into 2,638 phis in GORILLA's
MAKECITYSCAPE and the demotion below stored them all back: 189,936 bytes at
m32 -Os where main's is 47,769. D-EH1 (DEVIATE): MIR has no variadic
intrinsic, so one call per variable stands for `localescape`; exit: the
intrinsic in `llrm-mir` (owner splitkit-port, 2026-11-06).

Row D-EH2 (open): GORILLA m32 is 3.5%, 4.6%, 5.4% and 4.4% over main at
-O1, -O2, -O3 and -Os (50,709 against 48,994; 51,219 against 48,976; 51,609
against 48,976; 49,860 against 47,769), m16 1.8%, 2.7%, 3.3% and 2.3%
(42,540 against 41,768; 42,890 against 41,758; 43,130 against 41,758; 41,528
against 40,602). DRAWGORILLA (+187 instructions at m32 -Os), PLOTSHOT
(+117), DOSUN (+65) and MAKECITYSCAPE (+53) carry it. Owner splitkit-port,
by 2026-10-27, closed by the per-function listing diff against main; closes
when all eight are at or under main.

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
- Erases the `FRAME` calls.
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
