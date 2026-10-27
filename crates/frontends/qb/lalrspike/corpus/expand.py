"""A small pattern language for statement syntax, expanded into the sentences it allows.

    LINE [[STEP]({n},{n})]-[STEP]({n},{n})[,[{n}][,[{B|BF}][,{n}]]]

  [ ... ]   optional (nested ok)         {a|b}   one of the literal alternatives
  {n}       a slot of type n, below      anything else is literal

Slot types: n numeric expression, i integer expression, s string expression, v numeric variable,
t string variable, l label, f file number (with #), g file number without, a array name with (),
c coordinate-free "color" (numeric), k key number, q a quoted string literal.
"""

import itertools
import random

POOLS = {
    "n": ["x%", "1", "l&", "s!", "d#", "(x% + 1)", "arr%(1)", "-1", "100"],
    "i": ["x%", "3", "arr%(1)", "(x% * 2)"],
    "s": ["s$", '"abc"', "LEFT$(s$, 2)", "sa$(1)"],
    "v": ["x%", "l&", "s!", "d#", "arr%(1)", "r.a"],
    "t": ["s$", "sa$(1)", "r.s"],
    "l": ["lab"],
    "f": ["#1", "#x%", "#3"],
    "g": ["1", "x%"],
    "a": ["arr%()", "ars!()", "sa$()", "m%()"],
    "c": ["x%", "7", "15"],
    "k": ["1", "x%", "10"],
    "q": ['"abc"', '"MB T120"'],
}
# a value of the wrong kind, to make the slot's type fail
WRONG = {
    "n": ['"a"', "s$"],
    "i": ['"a"', "s$"],
    "s": ["1", "x%"],
    "v": ["1", '"a"', "s$"],
    "t": ["x%", "1", '"a"'],
    "l": ["1.5", '"a"'],
    "f": ['"a"', "s$"],
    "g": ['"a"', "s$"],
    "a": ["x%", "1"],
    "c": ['"a"', "s$"],
    "k": ['"a"'],
    "q": ["1", "x%"],
}


def parse(pattern):
    """Pattern -> tree: list of str | ("opt", tree) | ("alt", [str]) | ("slot", type)."""
    pos = 0

    def seq(until):
        nonlocal pos
        items, text = [], ""
        while pos < len(pattern) and pattern[pos] != until:
            c = pattern[pos]
            if c == "[":
                if text:
                    items.append(text)
                    text = ""
                pos += 1
                items.append(("opt", seq("]")))
                pos += 1
            elif c == "{":
                if text:
                    items.append(text)
                    text = ""
                end = pattern.index("}", pos)
                body = pattern[pos + 1:end]
                pos = end + 1
                items.append(("alt", body.split("|")) if "|" in body else ("slot", body))
            else:
                text += c
                pos += 1
        if text:
            items.append(text)
        return items

    tree = seq(None)
    return tree


def shapes(tree):
    """Every choice of optional groups and alternatives: lists of tokens str | ("slot", type)."""
    options = [[]]
    for item in tree:
        if isinstance(item, str):
            options = [o + [item] for o in options]
        elif item[0] == "slot":
            options = [o + [item] for o in options]
        elif item[0] == "alt":
            options = [o + [("slot", alt) if len(alt) == 1 and alt in POOLS else alt] for o in options for alt in item[1]]
        else:
            inner = shapes(item[1])
            options = [o + i for o in options for i in [[]] + inner]
    return options


def render(shape, values):
    out, n = [], 0
    for token in shape:
        if isinstance(token, tuple):
            out.append(values[n] if n < len(values) else POOLS[token[1]][0])
            n += 1
        else:
            out.append(token)
    return "".join(out)


def sentences(pattern, cap=80, seed=1):
    """The valid shapes with default slot values, then the shapes with each slot's other values one at a time,
    then the shapes with one slot given the wrong kind of value."""
    rng = random.Random(seed)
    shs = shapes(parse(pattern))
    # dedupe by rendered text
    base = {}
    for sh in shs:
        base.setdefault(render(sh, []), sh)
    items = list(base.items())
    if len(items) > cap:
        items = rng.sample(items, cap)
    out = [(text, "valid-shape") for text, _ in items]
    biggest = max(base.values(), key=lambda sh: sum(isinstance(t, tuple) for t in sh))
    slots = [t for t in biggest if isinstance(t, tuple)]
    for i, (_, kind) in enumerate(slots):
        for value in POOLS[kind][1:]:
            values = [POOLS[t[1]][0] for t in slots]
            values[i] = value
            out.append((render(biggest, values), f"value:{i}:{kind}"))
        for value in WRONG[kind]:
            values = [POOLS[t[1]][0] for t in slots]
            values[i] = value
            out.append((render(biggest, values), f"wrong:{i}:{kind}"))
    # an extra trailing argument, and a missing one
    full = render(biggest, [])
    out.append((full + ", 1", "extra-argument"))
    seen, uniq = set(), []
    for text, why in out:
        if text not in seen:
            seen.add(text)
            uniq.append((text, why))
    return uniq


if __name__ == "__main__":
    import sys
    for text, why in sentences(sys.argv[1]):
        print(f"{why:18} {text}")
