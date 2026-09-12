from pathlib import Path

from tests import corpus
from qbopt.model import mir
from qbopt.analysis import loops
from qbopt.frontend import blocks
from qbopt.objectfile import module
from qbopt.frontend import fppatches
from qbopt.objectfile import addends
from qbopt.optimize import transform


def test_hoisted_copy_keeps_its_explicit_source_dominating() -> None:
    # qrender drew 807 instead of 809 triangles: a matrix-pointer copy
    # moved above its source load because that source also preserved bits.
    found = corpus.loaded(Path("fixtures/regressions/d_faces-borland.obj"))
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    records = addends.canonical(fppatches.native_records(found, mapped.starts), found.seg, len(found.code))
    assert not isinstance(records, str)
    found = module.of(records)
    assert found is not None
    mapped = blocks.code_map(found)
    assert not isinstance(mapped, str)
    partition = list(blocks.partition(found, mapped))
    body = next(body for _, body in mir.bodies(found, partition) if body.entry == 0x38A)
    checked = []

    def check(stage: str, state: mir.MirBody) -> None:
        if stage != "r02-hoist":
            return
        dominators = loops.dominators(state.blocks, state.entry)
        sources = {
            value: (block.at, index)
            for block in state.blocks
            for index, op in enumerate(block.ops)
            for value in op.defines
        }
        for block in state.blocks:
            for index, op in enumerate(block.ops):
                for arg in op.args:
                    if not isinstance(arg, mir.Held) or arg.value not in op.merges or arg.value not in sources:
                        continue
                    source, position = sources[arg.value]
                    checked.append(op.at)
                    assert source in dominators[block.at], (op.at, source, block.at)
                    assert source != block.at or position < index

    transform.applied(body, found.dgroup, found.calls, blocks=partition, found=found, watch=check)
    assert checked, "the fixture must contain an explicit operand also named by merge metadata"
