"""What each operation requires of a register, so nothing else has to guess.

MIR names values, not registers -- `docs/variables.md` is the plan for
getting the last of the register names out of it -- and a pass that moves
code has no business choosing where a value lives. But the machine does
have requirements, and until they were written down they were scattered:
`transform._implicit` was a predicate a pass consulted to decide whether to
give up, `target.BASES` was a set applied ad hoc, and the rest lived
in whatever `select.emit` happened to encode.

They belong in one place, between MIR and the allocator: lowering says what
an instruction needs, and the allocator satisfies it or splits a live range
until it can. A pass says only that a value is live somewhere.

Two kinds:

**Fixed.** The instruction reads or writes a particular register and does
not name it. `imul word [k]` multiplies by ax and leaves dx:ax; `cwd`
extends ax into dx; a shift by a variable count takes it in cl. Nothing in
the operand list says so, which is exactly why a rename breaks it.

**A class.** Any register from a set. 16-bit addressing reaches memory
through bx, bp, si or di and nothing else -- `[dx+0Ah]` has no encoding --
so a value some instruction reaches a cell by lives in one of those.
"""

from dataclasses import dataclass

from iced_x86 import Register_

from qbopt.model import ir


@dataclass(frozen=True, slots=True)
class Insn:
    """One machine instruction, as the thing that emits it needs it.

    `what` is None where the bytes are carried rather than generated -- a
    barrier, the restore idiom, an emulated x87 site. `covers` says which of
    the original bytes it stands for, and `at` is where it began, which is
    what a fixup and a branch target are still keyed on.

    `op` is the MIR operation it came from. It is here because select.py,
    layout.py and omfwrite.py all still ask MIR questions of a machine
    instruction, and taking that away is a change to three modules rather
    than to this one. Nothing above LIR may read it.
    """

    at: int
    covers: "tuple[int, int] | None"
    what: "ir.Semantics | None"
    # Which values this instruction writes and reads, by id. Carried
    # because the allocator is a LIR pass and needs the interference graph:
    # without them it had to be handed the MIR body the instruction came
    # from, and read `op.defines` off that. Ids rather than values, because
    # that is what an operand names -- ir.py sits below mir.py.
    #
    # Flags are not here. They are one register nothing is placed in, and
    # every consumer of liveness dropped them again on the way past.
    defines: tuple[int, ...]
    uses: tuple[int, ...]
    # Registers this instruction destroys without naming any of them.
    # LLVM's register mask operand, which is how a call says what it does
    # to the register file: it lists what survives, and this lists what
    # does not, which is the same fact and the one an allocator asks for.
    #
    # The alternative is what was here before -- the raise inventing a
    # value per clobbered register, so a call "defined" six things. 114 of
    # nbody's 162 call defines were read by nothing, and each one still got
    # an interval, competed for a register and was spilled.
    clobbers: "frozenset[Register_]" = frozenset()
    # Registers it destroys only the upper half of: a 386 callee under the
    # 8086 convention keeps si and di, and nothing more of esi and edi.
    clobbers_high: "frozenset[Register_]" = frozenset()
    # Every disjoint range of BC's bytes the operation stands for, where
    # that is more than the one `covers` names. A folded site whose pushes
    # sit apart from its call is the only shape with any: `covers` cannot
    # say "these five bytes and those twelve" without also claiming what
    # lies between them. Carried here so `without` can see that dropping
    # this instruction would strand bytes it cannot hand to a neighbour.
    spread: tuple = ()
    op: object = None
    # Decoded source occurrence transferred at lowering. Machine provenance
    # begins here; MIR operations carry only the SourceMap identity.
    node: object = None
    # Which parallel copy this move belongs to, or None for an ordinary
    # one. A phi says several values arrive together on one edge, and the
    # moves it becomes are simultaneous: emitting them in the order they
    # were written is right only while none reads what an earlier one
    # overwrote. pressx-v-evt produced `r24 <- [bp-8]` and then
    # `r27 <- r24`, so one arm carried the wrong value.
    #
    # Owned here rather than inferred: every move in a group shares an
    # address with the others, and so does an ordinary move beside them.
    group: int | None = None
    # Values this instruction reads in a register it does not name. A
    # runtime routine takes its arguments in particular registers and
    # mentions none of them, so an occurrence cannot say it: without this
    # the allocation put B$ENRA's two arguments in si and di where the
    # routine reads cx and bx, and arrprm printed an array it was never
    # told about. By value id, like `uses`.
    requires: "tuple[tuple[ir.Held, Register_], ...]" = ()
    # And the ones it *writes* in a register it does not name. The sibling
    # of `requires`, and it needs to be its own field because the copy
    # goes the other way: a read is satisfied by a move in front of the
    # instruction and a write by one behind it. Put in `requires` instead,
    # the restore idiom's two halves each became a read with a move in
    # front, nine of them in lngmix's loop.
    delivers: "tuple[tuple[ir.Held, Register_], ...]" = ()
    # How wide each value is, for an instruction whose semantics name no
    # operand to say so. A folded site and the restore idiom are both
    # several instructions behind one node, so every consumer that asked
    # the semantics got nothing and defaulted to a word: the divide's
    # result was spilled two bytes wide and reloaded four out of the same
    # slot. The width is the value's, established once at the raise.
    widths: "tuple[tuple[int, int], ...]" = ()
    # Whether this instruction holds the relocated operand of the operation
    # it belongs to. None means nothing moved it and the operation answers
    # as it always did. A pass that lifts a symbolic memory operand onto an
    # instruction of its own sets True there and False on what it left
    # behind: the fixup follows the operand, and it is the only thing that
    # does -- which bytes the operation stands for and where it stands are
    # still the operation's own.
    symbol: "bool | None" = None
    # Allocator-owned stack reads can be deleted when their result is dead;
    # an arbitrary source-program memory read may have observable faults.
    spill_reload: bool = False
    # The allocator's own store putting a spilled value away. It writes that
    # slot and nothing else, which an inserted instruction cannot otherwise
    # say: it carries the `op` of whatever it stands beside, stores and all.
    spill_store: bool = False
    frame_adjust: bool = False
    # This instruction reconstructs a value at its use instead of preserving
    # it in a register or an allocator-owned spill slot. A stable source-cell
    # reconstruction may also be a ``spill_reload`` for ownership purposes.
    rematerialized: bool = False

    @property
    def inserted(self) -> bool:
        """Whether this occurrence owns no source bytes of its own.

        Lowering may expand one MIR operation into several instructions.  The
        followers share its source address but deliberately cover an empty
        range.  A restore is the exception: it is an idiom represented by
        one source node and must retain that node while being emitted.
        """
        idiom = (
            self.op is not None
            and isinstance(self.node, ir.Restore)
            and getattr(self.what, "op", None) is ir.Operation.RESTORE
        )
        return not idiom and self.covers is not None and self.covers[0] == self.covers[1]

    @property
    def source(self):
        """The MIR provenance this occurrence still represents, if any."""
        return None if self.inserted else self.op

    @property
    def id(self):
        if self.inserted and self.symbol is not True:
            return None
        return getattr(self.op, "id", None)

    @property
    def kind(self):
        """Program operation kind, distinct from selected machine form."""
        from qbopt.model import mir

        source = self.source
        if source is None:
            return mir._kind_of(self.what, (), ()) if self.what is not None else mir.Kind.NOTHING
        if source.kind is mir.Kind.DIVMOD and self.what is not None:
            return mir._kind_of(self.what, (), ())
        return source.kind

    @property
    def name(self) -> str:
        source = self.source
        if source is not None:
            return source.name
        return self.what.name if self.what is not None else ""

    @property
    def args(self) -> tuple:
        return getattr(self.source, "args", ())

    @property
    def results(self) -> tuple:
        return getattr(self.source, "results", ())

    @property
    def raised(self):
        return getattr(self.source, "raised", None)

    @property
    def extra_covers(self) -> tuple:
        """Disjoint source ranges beyond the primary LIR ownership anchor."""
        return tuple(span for span in self.spread if span != self.covers)

    @property
    def rewritten(self) -> bool:
        """Whether optimization changed the source-level operation."""
        from qbopt.model import mir

        return self.source is None or not self.source.source_backed or mir.rewritten(self.source)


@dataclass(frozen=True, slots=True)
class Phi:
    """One value that is two definitions above this block, by id."""

    result: int
    incoming: "tuple[tuple[int, int], ...]"  # (predecessor block, the value arriving)


@dataclass(frozen=True, slots=True)
class LirBlock:
    at: int
    insns: tuple[Insn, ...]
    succ: tuple[int, ...] = ()
    # Where two definitions of one value meet: the result, and which value
    # arrives on each predecessor's edge. Not instructions -- nothing is
    # emitted for a phi -- and liveness still has to know the block defines
    # the result, or it stays live around every path reaching its use.
    #
    # Carried in full rather than as results alone because eliminating them
    # is a pass, and it needs the edges: LLVM runs PHIElimination before
    # allocation for the same reason, replacing each with a copy at the end
    # of the predecessor it came from.
    phis: tuple["Phi", ...] = ()
    # The frontend's mark (mir.MirBlock.cold) or noreturn.cold's.
    cold: bool = False

    @property
    def arrives(self) -> tuple[int, ...]:
        """What this block defines before its first instruction."""
        return tuple(one.result for one in self.phis)


@dataclass(frozen=True, slots=True)
class LirBody:
    """One procedure, lowered. Blocks in the order they are emitted."""

    name: str
    entry: int
    blocks: tuple[LirBlock, ...]
    # What the raise saw each value in, by value id. The allocator's input,
    # not its answer, and the fallback for an operand it could not place.
    origin: "dict[int, Register_]"
    # Where the raise fixed a value, by value id.
    pins: "dict[int, Register_]"
    # Values supplied by the caller in registers rather than defined by an
    # instruction in this body.  Keeping this explicit is what lets the
    # verifier distinguish a real ABI input from a transform that lost a
    # definition.
    inputs: frozenset[int] = frozenset()
    # Exact execution counts proved in MIR for canonical loop headers.  LIR
    # preserves this program fact for measurement only; selection,
    # allocation, and scheduling do not use it to change generated code.
    # Headers without an entry retain the explicitly heuristic frequency
    # model in tools/quality.py.
    loop_trip_counts: tuple[tuple[int, int], ...] = ()
    ordered: bool = False
    noreturn: bool = False
    # Blocks keep the order they arrive in. A BASIC body with an error
    # handler says so: RESUME NEXT finds the statement after the faulting
    # address, which holds only while each statement's code is contiguous.
    source_order: bool = False

    @property
    def insns(self) -> "tuple[Insn, ...]":
        return tuple(one for block in self.blocks for one in block.insns)


def anchor(one: Insn) -> Insn:
    """Keep virtual dataflow and byte ownership for an elided machine op.

    Register allocation and the physical cleanup passes can prove that an
    instruction changes no machine state.  Its virtual definition is a
    separate fact: later opaque LIR may still name that value.  An anchor emits
    no bytes, retains ``defines``/``uses`` and ownership, and carries none of
    the physical side effects of the instruction it replaces.
    """
    from dataclasses import replace

    return replace(
        one,
        what=ir.Semantics(ir.Operation.NOTHING, "", (), ()),
        clobbers=frozenset(),
        clobbers_high=frozenset(),
        group=None,
        requires=(),
        delivers=(),
        symbol=False,
        spill_reload=False,
        spill_store=False,
        frame_adjust=False,
        rematerialized=False,
    )


def without(insns, drop, rewrite=None) -> "list[Insn]":
    """`insns` without the ones `drop` picks, their bytes given to a survivor.

    An identity copy emits nothing, but it may still stand for bytes BC
    wrote -- `mov bx,ax` is two of them -- and layout refuses a body it
    cannot account for every byte of. Two phases remove such copies: the
    coalescer, once both ends are one value, and the rewriter, once both
    land in one register. Which copies are identities is each phase's own
    question; what happens to the bytes is this.

    Backward and inside the block, past the instructions that stand for no
    bytes at all: forward would hand a block's first bytes to something a
    branch may enter after them. Where there is no such predecessor, or
    the spans do not meet, the copy stays rather than the bytes going to
    an instruction that does not stand for them.
    """
    from dataclasses import replace

    out: list[Insn] = []
    for one in insns:
        kept = rewrite(one) if rewrite is not None else one
        if not drop(kept):
            out.append(kept)
            continue
        if not one.covers or one.covers[0] == one.covers[1]:
            continue  # inserted: it stands for nothing
        if len(one.spread) > 1:
            # A neighbour is given a span, and a span cannot carry two.
            # The reused divide's copy stands for its own five bytes and
            # for the twelve of the push run the fold deleted; handed on,
            # those twelve were owned by nothing and layout refused the
            # body with `0x005f: 12 bytes between the ops are not
            # instructions`. It costs a `mov` into its own register.
            out.append(kept)
            continue
        where = next(
            (
                index
                for index in range(len(out) - 1, -1, -1)
                if out[index].covers and out[index].covers[0] != out[index].covers[1]
            ),
            None,
        )
        last = out[where] if where is not None else None
        if last is None or last.covers[1] != one.covers[0]:
            out.append(kept)  # nothing to give them to
            continue
        out[where] = replace(last, covers=(last.covers[0], one.covers[1]))
    if len(out) > 1:
        first = out[0]
        following = next(
            (index for index, one in enumerate(out[1:], 1) if one.covers and one.covers[0] < one.covers[1]), None
        )
        second = out[following] if following is not None else None
        if (
            drop(first)
            and first.covers
            and second is not None
            and len(first.spread) <= 1
            and len(second.spread) <= 1
            and first.covers[1] == second.covers[0]
        ):
            out[following] = replace(second, covers=(first.covers[0], second.covers[1]))
            out = out[1:]
    return out
