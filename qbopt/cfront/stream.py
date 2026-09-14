"""The stream owshim/cgshim.c writes: one code-generator call per line.

    n7 CGBinary O_PLUS n5 n6 TY_INTEGER     a call and the handle it returned
    - CGDone n7                             a call that returned nothing
    SYM y1 name="pal_now" attr=0x42 seg=11  a record the shim adds
"""

import re
from dataclasses import field
from dataclasses import dataclass

HANDLE = re.compile(r"[a-z]\d+")


@dataclass(frozen=True, slots=True)
class Record:
    line: int
    result: str | None
    call: str
    args: tuple[str, ...]
    fields: dict = field(default_factory=dict)


def parse(text: str) -> list[Record]:
    records = []
    for number, line in enumerate(text.splitlines(), 1):
        tokens = _split(line)
        if not tokens:
            continue
        result = None
        if tokens[0] == "-" or HANDLE.fullmatch(tokens[0]):
            result = None if tokens[0] == "-" else tokens[0]
            tokens = tokens[1:]
        args, fields = [], {}
        for one in tokens[1:]:
            key, equals, value = one.partition("=")
            if equals and key.isidentifier():
                fields[key] = _value(value)
            else:
                args.append(_value(one))
        records.append(Record(number, result, tokens[0], tuple(args), fields))
    return records


def _split(line: str) -> list[str]:
    """Space-separated tokens; a quoted string holds no quote, the shim escapes it."""
    tokens, start, end = [], 0, 0
    while start < len(line):
        if line[start] == " ":
            start += 1
            continue
        end = start
        while end < len(line) and line[end] != " ":
            if line[end] == '"':
                end = line.index('"', end + 1)
            end += 1
        tokens.append(line[start:end])
        start = end
    return tokens


def _value(token: str) -> str:
    if len(token) >= 2 and token[0] == '"' and token[-1] == '"':
        return re.sub(r"\\x([0-9a-f]{2})", lambda match: chr(int(match.group(1), 16)), token[1:-1])
    return token
