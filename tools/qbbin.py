"""QuickBASIC 4.5 "Fast Load and Save" programs (the binary .BAS): read, check, write, dump. The format is
docs/formats/qb-binary.md.

    python3 tools/qbbin.py FILE...            one line per file
    python3 tools/qbbin.py --names FILE       the name table and its hash chains
    python3 tools/qbbin.py --tokens FILE      every token of every block
    python3 tools/qbbin.py --opcodes          the opcode table: operand words, stack operands, QB's own listing
    python3 tools/qbbin.py --check FILE...    framing, line counts, and emit(parse(file)) == file

`parse(data)` returns a `Module` or raises `QbError`; `Module.emit()` returns the bytes; `tokens(code)` walks a code
block. Standard library only.
"""
from __future__ import annotations

import struct
import sys
from dataclasses import dataclass, field

MAGIC = 0xFC
BASE = 0x1E  # offsets in the name table, in the file header and in the code are counted from here
HASH_AT, SLOTS = 0x1C, 41
NAMES_AT = 0x74
GAP = 0x101
END_OF_CODE = (0x0009, 0x0008)

# opcode, operand kind, operands the keyword takes from the stack, the way QB lists it. Kinds: N words follow; s = u16 length,
# that many bytes, padded to even; d = u16 length, that many bytes; c = u16 count n, n - 1 more words; ? = not seen.
OPS_TEXT = r"""
000 0 0 LINE (imm = indent)
001 1 0 CONTINUATION LINE (operand = column)
004 2 0 LINE WITH LABEL (next, name)
006 0 0 : statement separator
007 ? 0 :
008 0 0 END OF CODE (second word)
009 0 0 END OF CODE (first word)
00a s 0 TEXT LINE (len, next, raw text)
00b 1 0 VARIABLE (name; imm = type)
00c 1 1 ASSIGN (name; imm = type)
00d 1 0 DECLARE VARIABLE (name)
00e 2 0 ARRAY ELEMENT / FUNCTION CALL (n, name)
00f 2 0 ARRAY ELEMENT ASSIGN (n, name)
010 2 0 DIM ARRAY (n, name)
011 1 0 .field
012 1 2 .field =
015 2 0 AS usertype (name, ctx)
016 2 0 AS basetype (type, ctx)
017 0 0 ?
018 0 0 default lower bound
019 1 0 TYPE element (name)
01a 0 0 SHARED modifier
01b 3 0 DEFtype (next, type|mask, mask)
01c 0 0 REDIM marker
01d 1 0 END TYPE (next)
01e 1 0 SHARED (ffff)
01f 1 0 STATIC (ffff)
020 2 0 TYPE (end, name)
021 1 0 $STATIC
022 1 0 $DYNAMIC
023 0 0 CONST
025 0 1 BYVAL a
026 2 1 DEF FN end (len, ffff)
027 0 1 COM(a)
028 1 1 ON a GOSUB
029 0 1 KEY(a)
02a 0 0 OFF
02b 0 0 ON
02c 0 0 STOP
02d 0 0 PEN
02e 0 0 PLAY
02f 0 1 PLAY(a)
030 0 1 SIGNAL(a)
031 0 1 STRIG(a)
032 0 0 TIMER
033 0 1 TIMER(a)
034 ? 0 :
035 ? 0 :
036 0 1 SEG a
037 2 0 CALL (argc, name)
038 2 0 call without CALL (argc, name)
03a 0 0 CASE ELSE
03b 0 1 CASE a
03c 0 2 CASE a TO b
03d 0 1 CASE IS = a
03e 0 1 CASE IS < a
03f 0 1 CASE IS > a
040 0 1 CASE IS <= a
041 0 1 CASE IS >= a
042 0 1 CASE IS <> a
043 0 1 CHAIN a
044 d 0 DECLARE (len, name, kind, nparams, params)
045 d 0 DEF FN (len, ...)
046 0 0 DO
047 1 1 DO UNTIL a
048 1 1 DO WHILE a
049 1 0 ELSE
04A 1 0 line-number operand of ELSE (name)
04C 0 0 ELSE
04d 1 1 ELSEIF a THEN
04e 0 0 END
04f 2 0 END DEF
050 0 0 END IF
051 0 0 END DEF
052 0 0 END SELECT
053 1 0 EXIT DO
054 1 0 EXIT FOR
055 1 0 EXIT DEF
056 2 3 FOR a = b TO c
057 2 3 FOR  = a TO b STEP c
058 d 0 FUNCTION (len, name, kind, nparams, params)
059 1 0 GOSUB
05a ? 0 GOSUB
05b 1 0 GOTO
05c ? 0 GOTO
05d 1 1 IF a THEN
05e 1 1 IF a THEN
05f 1 1 IF a THEN
060 1 1 IF a GOTO
061 1 1 IF a THEN
062 1 0 LOOP
063 1 1 LOOP UNTIL a
064 1 1 LOOP WHILE a
065 2 0 NEXT
066 2 1 NEXT a
067 1 0 ON ERROR GOTO
068 d 0 ON ... GOSUB (len, names)
069 d 0 ON ... GOTO (len, names)
06a 0 0 RESTORE
06b 1 0 RESTORE
06c 0 0 RESUME
06d 1 0 RESUME
06e 0 0 RESUME NEXT
06f 0 0 RETURN
070 1 0 RETURN
071 0 1 RUN a
072 1 0 RUN
073 0 0 RUN
074 1 1 SELECT CASE a
075 0 0 STOP
076 d 0 SUB (len, name, kind, nparams, params)
077 0 2 WAIT a, b
078 0 3 WAIT a, b, c
079 1 0 WEND
07a 1 1 WHILE a
07d 0 0 
07e 0 1 , , a
07f 0 1 , a
080 0 1 , a
081 0 2 (a, b)
082 0 2 STEP(a, b)
083 0 2 -(a, b)
084 0 2 -STEP(a, b)
085 0 1 FIELD a
086 0 2 , a AS b
087 0 0 
088 0 0 
089 c 0 INPUT (count, flags...)
08a 0 1 #a
08f 0 1 PRINT SPC(a)
090 0 1 PRINT TAB(a)
091 0 0 PRINT ,
092 0 0 
093 0 0 PRINT
094 0 1 PRINT a,
095 0 1 PRINT a;
096 0 1 PRINT a
097 s 0 ' comment (len, column, text)
098 0 0 
099 s 0 $INCLUDE (len, text)
09a 0 0 BEEP
09B 0 1 BLOAD a
09c 0 2 BLOAD a, b
09d 0 3 BSAVE a, b, c
09e 0 1 CHDIR a
09f 0 2 CIRCLE a, b
0a0 0 3 CIRCLE a, b, c
0a1 1 0 
0a2 1 0 
0a3 0 1 CLS a
0a4 1 0 
0a5 2 0 COMMON //
0a6 s 0 DATA (len, text)
0a7 0 1 DATE$ = a
0a8 0 0 DEF SEG
0a9 0 1 DEF SEG = a
0aa 0 1 DRAW a
0ab 0 1 ENVIRON a
0ac 1 0 
0ad 0 1 ERROR a
0ae 0 0 FILES
0af 0 1 FILES a
0B0 0 1 GET a
0B1 0 2 GET a, b
0b2 1 2 GET a, , b
0b3 1 3 GET a, b, c
0b4 0 2 GET a, b
0b5 1 2 
0b6 0 1 INPUT a
0b7 0 2 IOCTL a, b
0b8 1 0 KEY {
0b9 0 2 KEY a, b
0ba 0 1 KILL a
0bb 1 1 LINE a, , BF
0bc 1 2 LINE a, b, BF
0bd 1 2 LINE a, , BF, b
0be 1 3 LINE a, b, BF, c
0bf 0 0 LET
0c0 1 1 LINE INPUT a
0c1 1 0 
0c2 1 1 LOCK a
0c3 0 0 LPRINT
0c4 0 2 LSET b = a
0c5 0 3 MID$(c, a) = b
0c6 0 3 MID$(c, , a) = b
0c7 0 1 MKDIR a
0c8 0 2 NAME a AS b
0c9 1 2 OPEN a AS b
0ca 1 3 OPEN a AS b LEN = c
0cb 0 3 OPEN a, b, c
0cc 0 3 OPEN , a, b, c
0cd 0 0 OPTION BASE 0
0ce 0 0 OPTION BASE 1
0cf 0 2 OUT a, b
0d0 0 3 PAINT a, b, c
0D1 0 3 PAINT , a, b, c
0d2 0 0 PALETTE
0d3 0 2 PALETTE a, b
0d4 0 1 PALETTE USING a
0d5 0 2 PCOPY a, b
0d6 0 1 PLAY a
0d7 0 2 POKE a, b
0d8 0 1 PRESET a
0d9 0 2 PRESET a, b
0da 0 1 PSET a
0db 0 2 PSET a, b
0dc 0 1 PUT a
0dd 0 2 PUT a, b
0DE 1 2 PUT a, , b
0df 1 3 PUT a, b, c
0e0 0 0 RANDOMIZE
0e1 0 1 RANDOMIZE a
0e2 0 1 READ a
0e3 s 0 REM (len, text)
0e4 0 0 RESET
0e5 0 1 RMDIR a
0e6 0 2 RSET b = a
0e7 1 0 
0e8 0 2 SEEK a, b
0e9 0 0 SHELL
0ea 0 1 SHELL a
0eb 0 0 SLEEP
0ec 0 2 SOUND a, b
0ed 1 2 SWAP a, b
0ee 0 0 SYSTEM
0ef 0 1 TIME$ = a
0f0 0 0 TROFF
0f1 0 0 TRON
0f2 1 1 UNLOCK a
0f3 0 0 
0F4 0 0 VIEW
0f5 0 0 VIEW PRINT
0f6 0 2 VIEW PRINT a TO b
0F7 0 2 VIEW SCREEN
0f8 0 2 WIDTH a, b
0f9 0 1 WIDTH LPRINT a
0FA 0 2 WIDTH a, b
0fb 0 3 WINDOW (, a)-(b, c)
0fc 0 0 WINDOW
0fd 0 3 WINDOW SCREEN (, a)-(b, c)
0fe 0 0 WRITE
0ff 0 1 PRINT USING a;
100 0 1 + a
101 0 1 AND a
102 0 1 / a
103 0 1 = a
104 0 1 EQV a
105 0 1 ABS(a)
106 0 1 ASC(a)
107 0 1 ATN(a)
108 0 1 
109 0 1 CHR$(a)
10a 0 0 COMMAND$
10b 0 1 COS(a)
10c 0 0 CSRLIN
10d 0 1 CVD(a)
10e 0 1 CVDMBF(a)
10f 0 1 CVI(a)
110 0 1 CVL(a)
111 0 1 CVS(a)
112 0 1 CVSMBF(a)
113 0 0 DATE$
114 0 1 ENVIRON$(a)
115 0 1 EOF(a)
116 0 0 ERDEV
117 0 0 ERDEV$
118 0 0 ERL
119 0 0 ERR
11a 0 1 EXP(a)
11b 0 2 FILEATTR(a, b)
11c 0 1 FIX(a)
11d 0 1 FRE(a)
11e 0 0 FREEFILE
11f 0 1 HEX$(a)
120 0 0 INKEY$
121 0 1 INP(a)
122 0 1 INPUT$(a)
123 0 2 INPUT$(a, b)
124 0 2 INSTR(a, b)
125 0 3 INSTR(a, b, c)
126 0 1 INT(a)
127 0 1 IOCTL$(a)
128 0 1 LBOUND(a)
129 0 2 LBOUND(a, b)
12a 0 1 LCASE$(a)
12b 0 1 LTRIM$(a)
12c 0 2 LEFT$(a, b)
12d 1 1 LEN(a)
12e 0 1 LOC(a)
12f 0 1 LOF(a)
130 0 1 LOG(a)
131 0 1 LPOS(a)
132 0 2 MID$(a, b)
133 0 3 MID$(a, b, c)
134 0 1 MKD$(a)
135 0 1 MKDMBF$(a)
136 0 1 MKI$(a)
137 0 1 MKL$(a)
138 0 1 MKS$(a)
139 0 1 MKSMBF$(a)
13a 0 1 OCT$(a)
13b 0 1 PEEK(a)
13c 0 1 PEN(a)
13d 0 1 PLAY(a)
13e 0 2 PMAP(a, b)
13f 0 1 POINT(a)
140 0 2 POINT(a, b)
141 0 1 POS(a)
142 0 2 RIGHT$(a, b)
143 0 0 RND
144 0 1 RND(a)
145 0 1 RTRIM$(a)
146 0 1 SADD(a)
147 0 2 SCREEN(a, b)
148 0 3 SCREEN(a, b, c)
149 0 1 SEEK(a)
14a 0 1 SETMEM(a)
14b 0 1 SGN(a)
14c 0 1 SHELL(a)
14d 0 1 SIN(a)
14e 0 1 SPACE$(a)
14f 0 1 SQR(a)
150 0 1 STICK(a)
151 0 1 STR$(a)
152 0 1 STRIG(a)
153 0 2 STRING$(a, b)
154 0 1 TAN(a)
155 0 0 TIME$
156 0 0 TIMER
157 0 1 UBOUND(a)
158 0 2 UBOUND(a, b)
159 0 1 UCASE$(a)
15a 0 1 VAL(a)
15b 0 1 VARPTR(a)
15c 1 1 VARPTR$(a)
15d 0 1 VARSEG(a)
15e 0 1 >= a
15f 0 1 > a
160 0 1 \ a
161 0 1 IMP a
162 0 1 <= a
163 0 1 < a
164 0 0 INT LITERAL 0..63 (imm)
165 1 0 INT LITERAL
166 2 0 LONG LITERAL
167 1 0 &H INT LITERAL
168 2 0 &H LONG LITERAL
169 1 0 &O INT LITERAL
16a 2 0 &O LONG LITERAL
16b 2 0 SINGLE LITERAL
16c 4 0 DOUBLE LITERAL
16d s 0 STRING LITERAL (len, text)
16e 0 1 (a)
16f 0 1 MOD a
170 0 1 * a
171 0 1 <> a
172 0 0 
173 0 0 
174 0 1 NOT a
175 0 1 OR a
176 0 1 ^ a
177 0 1 - a
178 0 1 -a
179 0 1 XOR a
17a 0 0 UEVENT
17B 0 1 SLEEP a
17c 3 0 AS  * 8
17d 1 0 DIM
"""


class QbError(ValueError):
    pass


def _ops():
    out = {}
    for line in OPS_TEXT.strip().split("\n"):
        op, kind, arity, text = (line.split(" ", 3) + [""])[:4]
        out[int(op, 16)] = (kind if kind in "sdc?" else int(kind), int(arity), text)
    return out


OPS = _ops()


@dataclass
class Name:
    flags: int
    text: str
    next: int  # offset of the next name in this hash chain, 0 at its end


@dataclass
class Proc:
    name: str
    kind: int  # 1 SUB, 2 FUNCTION
    type: int  # FUNCTION result type: 1 integer, 2 long, 3 single, 4 double, 5 string
    flags: int  # 0x80 STATIC, 0x08 declared
    code: bytes
    trailer: bytes


@dataclass
class Module:
    header: bytes  # bytes 0 to 0x1B
    slots: list[int]  # the 41 hash buckets: offset of the newest name in each chain, 0 if empty
    names: list[Name]
    gap: bytes  # 257 bytes QB leaves as they were in memory; no program data
    code: bytes  # the main module's lines, ending 09 00 08 00
    trailer: bytes  # 16 bytes
    procs: list[Proc] = field(default_factory=list)
    start: int = 0x52  # the word at 0x70
    cache: int = 0  # the word at 0x72: offset of the name QB looked up last; 0 is as good

    def name_offsets(self) -> list[int]:
        out, at = [], NAMES_AT - BASE
        for n in self.names:
            out.append(at)
            at += 4 + len(n.text)
        return out

    def emit(self) -> bytes:
        names = b"".join(bytes([n.flags, len(n.text)]) + n.text.encode("latin-1") + struct.pack("<H", n.next) for n in self.names)
        end = NAMES_AT - BASE + len(names)
        head = bytearray(self.header)
        struct.pack_into("<H", head, 0x1A, end + GAP + 2)
        out = bytes(head) + struct.pack("<41H", *self.slots) + struct.pack("<HHH", end, self.start, self.cache) + names
        out += self.gap + struct.pack("<H", len(self.code)) + self.code + self.trailer
        for p in self.procs:
            out += b"\0" + struct.pack("<H", len(p.name)) + p.name.encode("latin-1") + bytes([p.kind, p.type, p.flags])
            out += struct.pack("<H", len(p.code)) + p.code + p.trailer
        return out

    def blocks(self) -> list[tuple[str, bytes, bytes]]:
        return [("(main)", self.code, self.trailer)] + [(p.name, p.code, p.trailer) for p in self.procs]


def parse(data: bytes) -> Module:
    if len(data) < NAMES_AT or data[0] != MAGIC:
        raise QbError("not a QuickBASIC 4.5 binary program")
    slots = list(struct.unpack_from("<41H", data, HASH_AT))
    end_rel, start, cache = struct.unpack_from("<HHH", data, 0x6E)
    end = BASE + end_rel
    names, at = [], NAMES_AT
    while at < end:
        flags, ln = data[at], data[at + 1]
        names.append(Name(flags, data[at + 2 : at + 2 + ln].decode("latin-1"), struct.unpack_from("<H", data, at + 2 + ln)[0]))
        at += 4 + ln
    if at != end:
        raise QbError("name table does not end where the header says")
    code_at = BASE + struct.unpack_from("<H", data, 0x1A)[0]
    if code_at != end + GAP + 2:
        raise QbError("code pointer is not 0x103 bytes past the names")
    size = struct.unpack_from("<H", data, code_at - 2)[0]
    code = data[code_at : code_at + size]
    trailer = data[code_at + size : code_at + size + 16]
    at = code_at + size + 16
    if len(code) != size or len(trailer) != 16:
        raise QbError("main module runs past the end of the file")
    procs = []
    while at < len(data):
        if data[at] != 0:
            raise QbError(f"procedure block at {at:#x} does not start with 0")
        ln = struct.unpack_from("<H", data, at + 1)[0]
        name = data[at + 3 : at + 3 + ln].decode("latin-1")
        q = at + 3 + ln
        kind, typ, flags = data[q : q + 3]
        size = struct.unpack_from("<H", data, q + 3)[0]
        pcode = data[q + 5 : q + 5 + size]
        ptrailer = data[q + 5 + size : q + 21 + size]
        if len(pcode) != size or len(ptrailer) != 16:
            raise QbError(f"procedure {name} runs past the end of the file")
        procs.append(Proc(name, kind, typ, flags, pcode, ptrailer))
        at = q + 21 + size
    return Module(data[:HASH_AT], slots, names, data[end : end + GAP], code, trailer, procs, start, cache)


@dataclass
class Token:
    at: int
    op: int
    imm: int
    operands: list  # words, or the bytes of a string / declaration body


def tokens(code: bytes) -> list[Token]:
    """The tokens of a code block. A word is opcode (low 10 bits) and imm (high 6); the operands follow it."""
    out, p = [], 0
    while p < len(code):
        (w,) = struct.unpack_from("<H", code, p)
        op, imm, at = w & 0x3FF, w >> 10, p
        p += 2
        kind = OPS.get(op, ("?",))[0]
        if kind == "?":
            raise QbError(f"unknown opcode {op:#x} at {at}")
        if isinstance(kind, int):
            ops = list(struct.unpack_from(f"<{kind}H", code, p))
            p += 2 * kind
        elif kind in "sd":
            (ln,) = struct.unpack_from("<H", code, p)
            ops = [code[p + 2 : p + 2 + ln]]
            p += 2 + ln + (ln & 1 if kind == "s" else 0)
        else:  # c
            (n,) = struct.unpack_from("<H", code, p)
            ops = list(struct.unpack_from(f"<{n}H", code, p))
            p += 2 * n
        if p > len(code):
            raise QbError(f"token at {at} runs past the end of its block")
        out.append(Token(at, op, imm, ops))
    return out


def line_count(toks: list[Token]) -> int:
    return sum(1 for t in toks if t.op in (0x000, 0x001, 0x004))


def check(data: bytes) -> list[str]:
    try:
        mod = parse(data)
    except (QbError, struct.error, ValueError) as err:
        return [str(err)]
    bad = []
    if mod.emit() != data:
        bad.append("emit(parse(file)) differs from the file")
    for name, code, trailer in mod.blocks():
        try:
            toks = tokens(code)
        except (QbError, struct.error) as err:
            bad.append(f"{name}: {err}")
            continue
        if tuple(t.op for t in toks[-2:]) != END_OF_CODE:
            bad.append(f"{name}: does not end 09 00 08 00")
        count = struct.unpack_from("<H", trailer, 8)[0]
        if line_count(toks) != count:
            bad.append(f"{name}: {line_count(toks)} lines, the trailer says {count}")
    return bad


def render(t: Token, names: dict[int, str]) -> str:
    kind, _, text = OPS[t.op]
    parts = [f"{t.at:5}  {t.op:03X}" + (f".{t.imm}" if t.imm else "    "), text.split(" (")[0] if text else ""]
    for o in t.operands:
        if isinstance(o, bytes):
            parts.append(repr(o))
        else:
            parts.append(f"@{names[o]}" if o in names else f"{o:04X}")
    return " ".join(parts)


def main(argv: list[str]) -> int:
    flags = {a for a in argv if a.startswith("--")}
    files = [a for a in argv if not a.startswith("--")]
    if "--opcodes" in flags:
        for op, (kind, arity, text) in sorted(OPS.items()):
            print(f"{op:03X}  words {kind}  takes {arity}  {text}")
        return 0
    if not files:
        print(__doc__, file=sys.stderr)
        return 2
    status = 0
    for path in files:
        data = open(path, "rb").read()
        if "--check" in flags:
            bad = check(data)
            print(f"{path}: {'ok' if not bad else '; '.join(bad[:5])}")
            status |= bool(bad)
            continue
        try:
            mod = parse(data)
        except QbError as err:
            print(f"{path}: {err}", file=sys.stderr)
            status = 1
            continue
        offs = dict(zip(mod.name_offsets(), (n.text for n in mod.names)))
        if "--names" in flags:
            for off, n in zip(mod.name_offsets(), mod.names):
                print(f"{off:04X}  flags {n.flags:02X}  next {n.next:04X}  {n.text}")
        elif "--tokens" in flags:
            for name, code, trailer in mod.blocks():
                print(f"-- {name}")
                for t in tokens(code):
                    print(render(t, offs))
        else:
            print(f"{path}: {len(mod.names)} names, main {len(mod.code)} bytes, {len(mod.procs)} procedures")
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
