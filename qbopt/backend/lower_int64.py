"""Legalize MIR's whole 64-bit integers for the 386 machine boundary.

MIR deliberately keeps a C ``long long`` as one eight-byte value.  The
optimizer therefore sees the source-language operation, while this module --
the first target-specific step -- gives the 16-bit ABI its two dword halves.
Ordinary lowering and allocation then see only widths the target can hold.
"""

from dataclasses import replace
from dataclasses import dataclass

from iced_x86 import Register

from qbopt.model import ir
from qbopt.model import mir
from qbopt.abi import runtime
from qbopt.backend import lower


@dataclass(frozen=True, slots=True)
class Legalized:
    body: mir.MirBody
    calls: dict[int, str]
    contracts: dict[int, runtime.Contract]
    inline: dict[int, tuple[bytes, ...]]


def _helper(name: str) -> runtime.Contract:
    return runtime.Contract(
        name=name,
        cleanup=0,
        control=runtime.Control.RETURNS,
        enters_user_code=False,
        raises_error=False,
        error_handling=False,
        writes=runtime.Memory.NONE,
        reads=runtime.Memory.NONE,
        clobbers=frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX, runtime.Reg.FLAGS}),
        established=True,
        evidence="qbopt's inline 386 int64 helper; operands and results follow Open Watcom's register ABI",
        inputs=frozenset({runtime.Reg.AX, runtime.Reg.BX, runtime.Reg.CX, runtime.Reg.DX}),
        clobbers_reached=True,
        i386=True,
    )


# __U8M's register ABI is EDX:EAX * ECX:EBX -> EDX:EAX.  This is straight
# line code at the call site (there is deliberately no RET).
_MUL = bytes.fromhex("66 50 66 52 66 f7 e1 66 89 c1 66 58 66 f7 e3 66 01 c1 66 58 66 f7 e3 66 01 ca")


# Restoring division over EDX:EAX / ECX:EBX.  Both helpers preserve BP, ESI,
# EDI and SP, return quotient in EDX:EAX and remainder in ECX:EBX, and contain
# no RET because a Callee with code is laid down at the call site.
_UDIV = bytes.fromhex(
    "55 89 e5 66 56 66 57 66 51 66 53 66 89 c6 66 89 d7 66 31 c0 66 31 d2 b9 40 00 "
    "66 d1 e6 66 d1 d7 66 d1 d0 66 d1 d2 36 66 3b 56 f4 77 09 72 15 36 66 3b 46 f0 "
    "72 0e 36 66 2b 46 f0 36 66 1b 56 f4 66 83 ce 01 49 75 d3 66 89 d1 66 89 c3 "
    "66 89 fa 66 89 f0 36 66 8b 76 fc 36 66 8b 7e f8 89 ec 5d"
)
_SDIV = bytes.fromhex(
    "55 89 e5 66 56 66 57 66 89 d6 66 31 ce 66 89 d7 66 85 d2 79 0a 66 f7 da 66 f7 d8 "
    "66 83 da 00 66 85 c9 79 0a 66 f7 d9 66 f7 db 66 83 d9 00 66 56 66 57 66 51 66 53 "
    "66 89 c6 66 89 d7 66 31 c0 66 31 d2 b9 40 00 66 d1 e6 66 d1 d7 66 d1 d0 66 d1 d2 "
    "36 66 3b 56 ec 77 09 72 15 36 66 3b 46 e8 72 0e 36 66 2b 46 e8 36 66 1b 56 ec "
    "66 83 ce 01 49 75 d3 66 89 d1 66 89 c3 66 89 fa 66 89 f0 36 66 83 7e f4 00 7d 0a "
    "66 f7 da 66 f7 d8 66 83 da 00 36 66 83 7e f0 00 7d 0a 66 f7 d9 66 f7 db 66 83 d9 00 "
    "36 66 8b 76 fc 36 66 8b 7e f8 89 ec 5d"
)


class _Legalizer:
    def __init__(self, body: mir.MirBody, calls: dict[int, str], contracts: dict[int, runtime.Contract]) -> None:
        self.body = body
        self.calls = dict(calls)
        self.contracts = dict(contracts)
        self.inline: dict[int, tuple[bytes, ...]] = {}
        values = {
            value
            for block in body.blocks
            for value in (
                *(phi.result for phi in block.phis),
                *(value for phi in block.phis for value in phi.incoming.values()),
                *(value for op in block.ops for value in (*op.defines, *op.uses)),
            )
        }
        self.next_value = max((one.id for one in values), default=0) + 1
        wide = {
            arg.value
            for block in body.blocks
            for op in block.ops
            for arg in (*op.args, *op.results)
            if isinstance(arg, mir.Held) and arg.width == 8
        }
        self.pairs = {value: (self.fresh(value.at), self.fresh(value.at)) for value in wide}

    def fresh(self, at: int, *, flags: bool = False) -> mir.Value:
        value = mir.Value(self.next_value, at, flags, self.next_value, 1)
        self.next_value += 1
        return value

    @staticmethod
    def _held(values: tuple[mir.Value, mir.Value]) -> tuple[mir.Held, mir.Held]:
        return mir.Held(values[0], 4), mir.Held(values[1], 4)

    def pair(self, arg: mir.Arg) -> tuple[mir.Arg, mir.Arg]:
        if isinstance(arg, mir.Held) and arg.width == 8:
            return self._held(self.pairs[arg.value])
        if isinstance(arg, mir.Const) and arg.width == 8:
            number = arg.n & 0xFFFF_FFFF_FFFF_FFFF
            return mir.Const(number & 0xFFFF_FFFF, 4), mir.Const(number >> 32, 4)
        raise lower.Unlowered(f"64-bit operand {arg!r} has no dword pair")

    @staticmethod
    def ref(ref: mir.MemRef, high: bool = False) -> mir.MemRef:
        if ref.width != 8:
            return ref
        if high and ref.addr is None:
            raise lower.Unlowered("an unlocated 64-bit cell cannot name its high dword")
        return replace(ref, width=4, addr=ref.addr.plus(4) if high else ref.addr)

    def op(
        self,
        source: mir.Op,
        kind: mir.Kind,
        args: tuple[mir.Arg, ...] = (),
        results: tuple[mir.Arg, ...] = (),
        *,
        defines: tuple[mir.Value, ...] | None = None,
        uses: tuple[mir.Value, ...] | None = None,
        loads: tuple[mir.MemRef, ...] = (),
        stores: tuple[mir.MemRef, ...] = (),
        test: mir.Kind | None = None,
        target: int | None = None,
        args_known: bool = True,
        reads_complete: bool = True,
        memory_complete: bool = True,
    ) -> mir.Op:
        if defines is None:
            defines = tuple(arg.value for arg in results if isinstance(arg, mir.Held))
        if uses is None:
            uses = tuple(dict.fromkeys(arg.value for arg in args if isinstance(arg, mir.Held)))
        return mir.Op(
            source.at,
            ir.Operation.NOTHING,
            "",
            defines,
            uses,
            loads=loads,
            stores=stores,
            kind=kind,
            test=test,
            args=args,
            results=results,
            target=target,
            id=next(mir._IDS),
            args_known=args_known,
            reads_complete=reads_complete,
            memory_complete=memory_complete,
        )

    def binary(self, source: mir.Op) -> list[mir.Op]:
        left = self.pair(source.args[0])
        right = self.pair(source.args[1])
        out = self.pair(source.results[0])
        if source.kind in (mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR):
            return [
                self.op(source, source.kind, (left[0], right[0]), (out[0],)),
                self.op(source, source.kind, (left[1], right[1]), (out[1],)),
            ]
        carry = self.fresh(source.at, flags=True)
        if source.kind is mir.Kind.ADD:
            low, high = mir.Kind.ADD, mir.Kind.ADD_CARRY
        elif source.kind is mir.Kind.SUB:
            low, high = mir.Kind.SUB, mir.Kind.SUB_BORROW
        else:
            raise lower.Unlowered(f"64-bit {source.kind} at {source.at:#x}")
        first = self.op(source, low, (left[0], right[0]), (out[0],), defines=(out[0].value, carry))
        second = self.op(
            source,
            high,
            (left[1], right[1]),
            (out[1],),
            defines=(out[1].value,),
            uses=(left[1].value, right[1].value, carry),
        )
        return [first, second]

    def shift(self, source: mir.Op) -> list[mir.Op]:
        value = self.pair(source.args[0])
        out = self.pair(source.results[0])
        count = source.args[1]
        if not isinstance(count, mir.Const):
            raise lower.Unlowered(f"variable 64-bit shift at {source.at:#x}")
        amount = count.n & 63
        if amount == 0:
            return [
                self.op(source, mir.Kind.COPY, (value[0],), (out[0],)),
                self.op(source, mir.Kind.COPY, (value[1],), (out[1],)),
            ]
        if source.kind is mir.Kind.SHL:
            if amount >= 32:
                return [
                    self.op(source, mir.Kind.COPY, (mir.Const(0, 4),), (out[0],)),
                    self.op(source, mir.Kind.SHL, (value[0], mir.Const(amount - 32, 1)), (out[1],)),
                ]
            a, b = self.fresh(source.at), self.fresh(source.at)
            return [
                self.op(source, mir.Kind.SHL, (value[1], mir.Const(amount, 1)), (mir.Held(a, 4),)),
                self.op(source, mir.Kind.SHR, (value[0], mir.Const(32 - amount, 1)), (mir.Held(b, 4),)),
                self.op(source, mir.Kind.OR, (mir.Held(a, 4), mir.Held(b, 4)), (out[1],)),
                self.op(source, mir.Kind.SHL, (value[0], mir.Const(amount, 1)), (out[0],)),
            ]
        high_kind = mir.Kind.SAR if source.kind is mir.Kind.SAR else mir.Kind.SHR
        if amount >= 32:
            return [
                self.op(source, high_kind, (value[1], mir.Const(amount - 32, 1)), (out[0],)),
                self.op(source, high_kind, (value[1], mir.Const(31, 1)), (out[1],))
                if source.kind is mir.Kind.SAR
                else self.op(source, mir.Kind.COPY, (mir.Const(0, 4),), (out[1],)),
            ]
        a, b = self.fresh(source.at), self.fresh(source.at)
        return [
            self.op(source, mir.Kind.SHR, (value[0], mir.Const(amount, 1)), (mir.Held(a, 4),)),
            self.op(source, mir.Kind.SHL, (value[1], mir.Const(32 - amount, 1)), (mir.Held(b, 4),)),
            self.op(source, mir.Kind.OR, (mir.Held(a, 4), mir.Held(b, 4)), (out[0],)),
            self.op(source, high_kind, (value[1], mir.Const(amount, 1)), (out[1],)),
        ]

    def call_helper(self, source: mir.Op, name: str, code: bytes) -> list[mir.Op]:
        left = self.pair(source.args[0])
        right = self.pair(source.args[1])
        quotient = self.pair(source.results[0])
        results = quotient
        if len(source.results) == 2:
            results += self.pair(source.results[1])
        prefix = []
        arguments = []
        for arg in (left[0], right[0], right[1], left[1]):
            if isinstance(arg, mir.Held):
                arguments.append(arg)
                continue
            value = self.fresh(source.at)
            held = mir.Held(value, 4)
            prefix.append(self.op(source, mir.Kind.COPY, (arg,), (held,)))
            arguments.append(held)
        args = tuple(arguments)
        made = self.op(source, mir.Kind.CALL, args, results)
        self.calls[made.at] = name
        self.contracts[made.at] = _helper(name)
        self.inline[made.at] = (code,)
        if len(results) == 4:
            origin = dict(self.body.origin)
            origin[results[2].value] = Register.EBX
            origin[results[3].value] = Register.ECX
            self.body = replace(self.body, origin=origin)
        return [*prefix, made]

    def compare(self, source: mir.Op) -> list[mir.Op]:
        if not source.defines or not source.defines[0].flags:
            raise lower.Unlowered(f"64-bit comparison at {source.at:#x} defines no condition")
        flag = source.defines[0]
        tests = {
            op.test for block in self.body.blocks for op in block.ops if flag in op.uses and op.kind is mir.Kind.BRANCH
        }
        if not tests or not tests <= {mir.Kind.EQ, mir.Kind.NE}:
            raise lower.Unlowered(f"64-bit comparison at {source.at:#x} needs {tests}, not equality")
        left, right = self.pair(source.args[0]), self.pair(source.args[1])
        low, high, joined = self.fresh(source.at), self.fresh(source.at), self.fresh(source.at)
        return [
            self.op(source, mir.Kind.XOR, (left[0], right[0]), (mir.Held(low, 4),)),
            self.op(source, mir.Kind.XOR, (left[1], right[1]), (mir.Held(high, 4),)),
            self.op(
                source,
                mir.Kind.OR,
                (mir.Held(low, 4), mir.Held(high, 4)),
                (mir.Held(joined, 4),),
                defines=(joined, flag),
            ),
        ]

    def operation(self, source: mir.Op) -> list[mir.Op]:
        wide = any(isinstance(arg, (mir.Held, mir.Const)) and arg.width == 8 for arg in (*source.args, *source.results))
        if not wide:
            return [source]
        if source.kind is mir.Kind.LOAD:
            low, high = self.pair(source.results[0])
            ref = source.args[0].ref
            return [
                self.op(source, mir.Kind.LOAD, (mir.Cell(self.ref(ref)),), (low,), loads=(self.ref(ref),)),
                self.op(source, mir.Kind.LOAD, (mir.Cell(self.ref(ref, True)),), (high,), loads=(self.ref(ref, True),)),
            ]
        if source.kind is mir.Kind.STORE:
            low, high = self.pair(source.args[0])
            ref = source.results[0].ref
            return [
                self.op(source, mir.Kind.STORE, (low,), (mir.Cell(self.ref(ref)),), stores=(self.ref(ref),)),
                self.op(
                    source,
                    mir.Kind.STORE,
                    (high,),
                    (mir.Cell(self.ref(ref, True)),),
                    stores=(self.ref(ref, True),),
                ),
            ]
        if source.kind in (mir.Kind.ADD, mir.Kind.SUB, mir.Kind.AND, mir.Kind.OR, mir.Kind.XOR) and source.results:
            return self.binary(source)
        if source.kind is mir.Kind.SUB and not source.results:
            return self.compare(source)
        if source.kind in (mir.Kind.SHL, mir.Kind.SHR, mir.Kind.SAR):
            return self.shift(source)
        if source.kind is mir.Kind.MUL:
            return self.call_helper(source, "__U8M", _MUL)
        if source.kind in (mir.Kind.DIVMOD, mir.Kind.UDIVMOD):
            signed = source.kind is mir.Kind.DIVMOD
            return self.call_helper(source, "__I8D" if signed else "__U8D", _SDIV if signed else _UDIV)
        if source.kind in (mir.Kind.ZERO_EXTEND, mir.Kind.SIGN_EXTEND):
            low, high = self.pair(source.results[0])
            arg = source.args[0]
            low_kind = source.kind if getattr(arg, "width", 4) < 4 else mir.Kind.COPY
            made = [self.op(source, low_kind, (arg,), (low,))]
            made.append(
                self.op(source, mir.Kind.SAR, (low, mir.Const(31, 1)), (high,))
                if source.kind is mir.Kind.SIGN_EXTEND
                else self.op(source, mir.Kind.COPY, (mir.Const(0, 4),), (high,))
            )
            return made
        if source.kind is mir.Kind.COPY:
            incoming, outgoing = self.pair(source.args[0]), self.pair(source.results[0])
            return [
                self.op(source, mir.Kind.COPY, (incoming[0],), (outgoing[0],)),
                self.op(source, mir.Kind.COPY, (incoming[1],), (outgoing[1],)),
            ]
        if source.kind is mir.Kind.ARG:
            low, high = self.pair(source.args[0])
            refs = source.stores
            high_stores = tuple(self.ref(one, True) for one in refs)
            low_stores = tuple(self.ref(one) for one in refs)
            return [
                self.op(source, mir.Kind.ARG, (high,), stores=high_stores),
                self.op(source, mir.Kind.ARG, (low,), stores=low_stores),
            ]
        if source.kind is mir.Kind.CALL:
            results = self.pair(source.results[0])
            defines = tuple(one for one in source.defines if one != source.results[0].value) + tuple(
                one.value for one in results
            )
            return [replace(source, defines=defines, results=results)]
        if source.kind is mir.Kind.RETURN:
            args = self.pair(source.args[0])
            return [replace(source, args=args, uses=tuple(one.value for one in args))]
        raise lower.Unlowered(f"64-bit {source.kind} at {source.at:#x} has no target legalization")

    def run(self) -> Legalized:
        blocks = []
        for block in self.body.blocks:
            phis = []
            for phi in block.phis:
                if phi.result not in self.pairs:
                    phis.append(phi)
                    continue
                low, high = self.pairs[phi.result]
                phis += [
                    mir.Phi(low, {at: self.pairs[value][0] for at, value in phi.incoming.items()}),
                    mir.Phi(high, {at: self.pairs[value][1] for at, value in phi.incoming.items()}),
                ]
            ops = tuple(made for source in block.ops for made in self.operation(source))
            blocks.append(replace(block, phis=tuple(phis), ops=ops))
        body = replace(self.body, blocks=tuple(blocks))
        problems = mir.verify(body)
        if problems:
            raise lower.Unlowered("invalid int64 legalization: " + "; ".join(problems))
        return Legalized(body, self.calls, self.contracts, self.inline)


def expanded(
    body: mir.MirBody,
    calls: dict[int, str] | None = None,
    contracts: dict[int, runtime.Contract] | None = None,
) -> Legalized:
    """Split every eight-byte integer into ABI dwords, if the body has one."""
    has_wide = any(
        isinstance(arg, (mir.Held, mir.Const)) and arg.width == 8
        for block in body.blocks
        for op in block.ops
        for arg in (*op.args, *op.results)
    )
    if not has_wide:
        return Legalized(body, dict(calls or {}), dict(contracts or {}), {})
    return _Legalizer(body, calls or {}, contracts or {}).run()
