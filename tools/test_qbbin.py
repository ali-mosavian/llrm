"""tools/qbbin.py reads and writes QuickBASIC 4.5 binary programs as docs/formats/qb-binary.md says. FIXTURES are what QB 4.5
itself saved ("Fast Load and Save") for the program beside each: nothing of Microsoft's but the format. Against the QB 4.5 kit
(QB45_DIR) every shipped binary program must read, check and write back byte for byte."""
import base64
import importlib.util
import os
import struct
import sys
import zlib
from pathlib import Path

import pytest

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("qbbin", HERE / "qbbin.py")
qbbin = importlib.util.module_from_spec(spec)
sys.modules["qbbin"] = qbbin
spec.loader.exec_module(qbbin)

FIXTURES = {
    "print_ab": ('PRINT "AB"\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD//+RjAxUB2EMQQyjgIEPiHMZmRgcnaYxcDJwAAMbAiAhzsQIAJ"
          "N+ERI="),
    "assign": ('x = 5\r\ny = x + 1\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD///JjAwkgjCGaAIqEhiCQBRjBYioZBgFUkCcIsoDDDoGBm4gmc"
          "LKwMgDDEZOBg5gFEAAE1glEyMAhlcTWw=="),
    "labels": ('l1: PRINT 1\r\nl2: PRINT 2\r\nl3: GOTO l1\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD///ZjAxkgzCGGIYkLOIZDEFAkoUpxxBMGoFJY4ZRoMTAAoyGMI"
          "YU1mlAlhgw+FI4Qaz//5MYooHinAwcDExAHggwg3UwMQIAR7MWug=="),
    "defs": ('DEFINT A\r\nPRINT 1\r\nDEFSTR B\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD///JjAwMYQzRDNQECQxBIIrREUQ4MYwCOSCWZhABRgJDAwNDCu"
          "s0MP//f1YGBgdOBg4giwWI//9nBqtmYgQAYeYS+A=="),
    "sub": ('DECLARE SUB S (x%)\r\nS 1\r\nSUB S (x%) STATIC\r\nPRINT x%\r\nEND SUB\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD///JjAxEgTA4K5qg2gSGICDpwBgMJBkrGEaBAhC7ACMAGIiMjK"
          "AAZAKFegqrBTBawhg4GTiAEQEBTGD1QGlGhmBGhh1KQE4ZhkZulmiGaUA6EEUrM1AEaAAPAGJ4ITw="),
    "types": ('TYPE T1\r\nx AS INTEGER\r\nEND TYPE\r\nTYPE T2\r\ny AS INTEGER\r\nEND TYPE\r\n',
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD///5jAxwEMaQyEAYxDCkE1CRwxAEophCDIEkYwWYaQRiVjKMAg"
          "sgVmCQAAY2A4MkMDDFgFHDBGTLMsiBZYzAkSAJDGSEzP//nAwcQPL/fxYgyQY2h4kRAEUeFiQ="),
    "comment": ("PRINT 1 ' hi\r\n'==========\r\n",
          "eNr7w8DIwMPQyNjEyMbAyMTMwsohIPD/vwrD//+RjAxUB2EMQQyjgEEBiFNYpzFMZ2Bl4GBQyMgE8kFsBgZeLl"
          "sGTqDYfyhgAqtnYgQAek8TIA=="),
}


def fixture(name):
    return zlib.decompress(base64.b64decode(FIXTURES[name][1]))


def words(*w):
    return b"".join(struct.pack("<H", x) for x in w)


@pytest.mark.parametrize("name", sorted(FIXTURES))
def test_a_program_qb_saved_checks_and_writes_back_byte_for_byte(name):
    data = fixture(name)
    assert qbbin.check(data) == []
    assert qbbin.parse(data).emit() == data


def test_names_are_records_chained_from_the_hash_table():
    mod = qbbin.parse(fixture("assign"))
    assert [(n.flags, n.text, n.next) for n in mod.names] == [(0, "x", 0), (0, "y", 0)]
    offs = mod.name_offsets()
    assert offs == [0x56, 0x5B]
    assert mod.slots[ord("x") & 0x1F] == 0x56 and mod.slots[ord("y") & 0x1F] == 0x5B  # one letter: its code & 0x1F
    assert sum(1 for s in mod.slots if s) == 2


def test_code_is_postfix_tokens_a_word_of_opcode_and_immediate():
    toks = qbbin.tokens(qbbin.parse(fixture("assign")).code)
    got = [(hex(t.op), t.imm, t.operands) for t in toks]
    assert got == [("0x0", 0, []), ("0x164", 5, []), ("0xc", 0, [0x56]),         # x = 5
                   ("0x0", 0, []), ("0xb", 0, [0x56]), ("0x164", 1, []), ("0x100", 0, []), ("0xc", 0, [0x5B]),  # y = x + 1
                   ("0x9", 0, []), ("0x8", 0, [])]


def test_a_string_literal_is_a_length_and_padded_bytes():
    toks = qbbin.tokens(qbbin.parse(fixture("print_ab")).code)
    assert [(hex(t.op), t.operands) for t in toks[1:3]] == [("0x16d", [b"AB"]), ("0x96", [])]


def test_a_comment_is_a_length_a_column_and_text_with_runs_packed():
    rems = [t.operands[0] for t in qbbin.tokens(qbbin.parse(fixture("comment")).code) if t.op == 0x97]
    assert rems == [struct.pack("<H", 8) + b" hi",    # the column the ' stood in, then the text
                    struct.pack("<H", 0) + b"\x0d\x0a="]  # 0Dh, a run of 10, the character: ten '='


def test_labels_defs_and_types_are_linked_lists_headed_in_the_trailer():
    """The trailer's four words head lists whose links are the first operand of each entry: offsets in the code block of the next
    entry's operand, FFFF at the end. QB lists a program wrongly if they are not there (a DEFINT that comes out at the end)."""
    for name, slot, ops in (("labels", 0, (0x004,)), ("defs", 1, (0x01B,)), ("types", 2, (0x020, 0x01D))):  # TYPE links to its END TYPE
        mod = qbbin.parse(fixture(name))
        toks = qbbin.tokens(mod.code)
        entries = [(t.at + 2, t.operands[0]) for t in toks if t.op in ops]
        head = struct.unpack_from("<H", mod.trailer, 2 * slot)[0]
        seen, at = [], head
        while at != 0xFFFF:
            seen.append(at)
            at = dict(entries)[at]
        assert seen == [e[0] for e in entries], (name, seen, entries)


def test_a_procedure_block_carries_its_name_kind_flags_and_lines():
    mod = qbbin.parse(fixture("sub"))
    (proc,) = mod.procs
    assert (proc.name, proc.kind, proc.type) == ("S", 1, 0)
    assert proc.flags & 0x80 and proc.flags & 0x08  # STATIC, and a DECLARE exists
    assert struct.unpack_from("<H", proc.trailer, 8)[0] == qbbin.line_count(qbbin.tokens(proc.code)) == 3


def test_what_qb_writes_for_a_program_this_writes_for_the_same_tokens():
    ref = qbbin.parse(fixture("print_ab"))
    code = words(0, 0x016D, 2) + b"AB" + words(0x96, 9, 8)
    trailer = words(0xFFFF, 0xFFFF, 0xFFFF, 0xFFFF, 1, 0, 0) + bytes([2, 1])
    mine = qbbin.Module(ref.header, [0] * 41, [], bytes(qbbin.GAP), code, trailer)
    assert mine.emit() == fixture("print_ab")


@pytest.mark.parametrize("damage", [
    lambda d: b"\x00" + d[1:],
    lambda d: d[:100],
    lambda d: d[:-3],
    lambda d: d[:0x1A] + b"\x00\x00" + d[0x1C:],
])
def test_what_is_not_a_qb_binary_program_is_refused(damage):
    assert qbbin.check(damage(fixture("assign")))


def programs():
    root = os.environ.get("QB45_DIR")
    if root and (Path(root) / "INC").is_dir():
        yield from sorted(p for p in (Path(root) / "INC").glob("*.BAS") if p.read_bytes()[:1] == b"\xfc")


@pytest.mark.parametrize("path", list(programs()) or [pytest.param(None, marks=pytest.mark.skip("no QB45_DIR"))])
def test_every_shipped_binary_program_checks_and_writes_back(path):
    data = path.read_bytes()
    assert qbbin.check(data) == []
    assert qbbin.parse(data).emit() == data
