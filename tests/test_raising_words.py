"""Unused preserved bits must not make an INTEGER load depend on its old value."""

from dataclasses import replace

from iced_x86 import Register

from qbopt.frontend import raising_words
from qbopt.model import ir, mir


def body_with_reader(width=2):
    old, result, output = (mir.Value(index, index) for index in (1, 2, 3))
    copy = mir.Op(2, ir.Operation.MOVE, "", (result,), (old,),
                  kind=mir.Kind.COPY, args=(mir.Const(7, 2),),
                  results=(mir.Held(result, 2),), merges={old: result})
    read = mir.Op(3, ir.Operation.MOVE, "", (output,), (result,),
                  kind=mir.Kind.COPY, args=(mir.Held(result, width),),
                  results=(mir.Held(output, width),))
    return mir.MirBody(0, (mir.MirBlock(0, (), (copy, read), ()),)), old, result


def test_unused_upper_word_does_not_keep_the_old_definition():
    """IVARM's INTEGER branch load kept its redundant counter alive through a dead upper half."""
    body, old, _ = body_with_reader()
    first = raising_words.scalar(body).blocks[0].ops[0]
    assert not first.merges and old not in first.uses


def test_wide_reader_keeps_the_preserved_upper_word():
    body, old, result = body_with_reader(4)
    assert raising_words.scalar(body).blocks[0].ops[0].merges == {old: result}


def test_unknown_reader_keeps_the_preserved_upper_word():
    body, old, result = body_with_reader()
    first, read = body.blocks[0].ops
    read = replace(read, kind=mir.Kind.OPAQUE, args=(), results=())
    body = replace(body, blocks=(replace(body.blocks[0], ops=(first, read)),))
    assert raising_words.scalar(body).blocks[0].ops[0].merges == {old: result}


def test_body_exit_keeps_the_preserved_upper_word():
    body, old, result = body_with_reader()
    body = replace(body, origin={old: Register.EAX, result: Register.EAX})
    assert raising_words.scalar(body).blocks[0].ops[0].merges == {old: result}


def test_wide_phi_reader_keeps_the_preserved_upper_word():
    body, old, result = body_with_reader(4)
    first, read = body.blocks[0].ops
    joined = mir.Value(4, 4)
    read = replace(read, args=(mir.Held(joined, 4),), uses=(joined,))
    body = replace(body, blocks=(mir.MirBlock(0, (), (first,), (4,)),
                                 mir.MirBlock(4, (mir.Phi(joined, {0: result}),), (read,), ())))
    assert raising_words.scalar(body).blocks[0].ops[0].merges == {old: result}
