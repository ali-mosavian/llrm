"""Borland C's far/near/huge/pascal/cdecl/interrupt, rewritten for clang on already-preprocessed text.

`far` is read from its neighbours, as a Borland declarator reads it:
  T far *p         the pointee is in the far space      -> T __attribute__((address_space(1))) *p
  T * far p        the pointer object is far            -> same attribute, at file scope; dropped in a body (a local is on the stack)
  T far name       the object is far                    -> same at file scope; dropped in a body
  T far name(...)  the function is called far           -> dropped (kept as a mark for the call distance)
  (far *fp)(...)   a far function pointer               -> dropped
`T near *p` is a near pointer (address space 0) so it stays near under a far-data model; any other `near`, and pascal's
neighbours cdecl, interrupt and the other modifiers, are dropped (pascal becomes clang's stdcall).
"""
import re
TOKEN = re.compile(r'''(?P<ws>\s+)|(?P<line>\#[^\n]*)|(?P<str>"(?:\\.|[^"\\])*")|(?P<chr>'(?:\\.|[^'\\])*')|(?P<num>\.?\d(?:[eEpP][+-]|[\w.])*)|(?P<id>[A-Za-z_]\w*)|(?P<p>->|\+\+|--|<<=|>>=|\.\.\.|[-+*/%&|^<>=!]=|<<|>>|&&|\|\||.)''', re.S)
PASCAL = {"pascal", "__pascal", "_pascal"}
FAR = {"far", "__far", "_far", "huge", "__huge", "_huge"}
NEAR = {"near", "__near", "_near"}
DROP = {"pascal", "__pascal", "_pascal", "cdecl", "__cdecl", "_cdecl", "interrupt", "__interrupt", "_interrupt",
        "_export", "__export", "_loadds", "__loadds", "_saveregs", "__saveregs", "_fastcall", "__fastcall", "_stdcall", "__stdcall",
        "_Cdecl", "_Pascal", "_Far", "_Near", "_Huge", "_FAR", "_NEAR", "_HUGE"}
ATTR = "__attribute__((address_space(1)))"
HUGE_ATTR = "__attribute__((address_space(3)))"
NEAR_ATTR = "__attribute__((address_space(0)))"

def split_declarators(toks, out, sig):
    """`T far *a, far *b;` at statement level: a declarator list shares one base type, so a `far` that restarts it becomes its own
    declaration (`T far *a; T far *b;`). Only where the comma is outside parentheses and brackets."""
    return out

def attr(word):
    return HUGE_ATTR if word.strip("_") == "huge" else ATTR


def rewrite(text):
    text = text.replace("\x1a", "")
    text = re.sub(r'#pragma\s+aux\s+(\w+)\s+"([^"]+)"', r'#pragma redefine_extname \1 \2', text)
    text = inline_asm(text)
    toks = [(m.lastgroup, m.group()) for m in TOKEN.finditer(text)]
    sig = [i for i, (k, _) in enumerate(toks) if k not in ("ws",)]
    out = [t for _, t in toks]
    depth = 0
    paren = 0
    start = 0  # index in sig of the first token of the current statement
    for n, i in enumerate(sig):
        k, t = toks[i]
        prev = toks[sig[n - 1]][1] if n else ""
        nxt = toks[sig[n + 1]][1] if n + 1 < len(sig) else ""
        nxt2 = toks[sig[n + 2]][1] if n + 2 < len(sig) else ""
        nxtk = toks[sig[n + 1]][0] if n + 1 < len(sig) else ""
        if k == "p":
            if t == "{": depth += 1; start = n + 1
            elif t == "}": depth -= 1; start = n + 1
            elif t == ";" and paren == 0: start = n + 1
            elif t in "([": paren += 1
            elif t in ")]": paren -= 1
            elif t == "," and paren == 0 and depth >= 0:
                # a far declarator after a comma: restart the declaration with the base type written before the first declarator
                j = n + 1
                if j < len(sig) and toks[sig[j]][1] in FAR and j + 1 < len(sig) and toks[sig[j + 1]][1] == "*":
                    base = []
                    for m in range(start, n):
                        tk = toks[sig[m]][1]
                        if tk in FAR or tk == "*" or (toks[sig[m]][0] == "id" and m + 1 < n and toks[sig[m + 1]][1] in ("*", "[", "=", ";", ",") and tk not in ("unsigned", "signed", "long", "short", "int", "char", "const", "volatile", "struct", "union", "enum", "float", "double", "void")):
                            break
                        base.append(out[sig[m]] if out[sig[m]] else tk)
                    out[i] = "; " + " ".join(b for b in base if b)
            continue
        if k != "id": continue
        if t in PASCAL:
            out[i] = "__attribute__((stdcall))"
        elif t in DROP:
            out[i] = ""
        elif t in NEAR:
            # `T near *p` is a near pointer whatever the model's default is; any other `near` is a mark on a function or object.
            out[i] = NEAR_ATTR if nxt == "*" and prev != "(" else ""
        elif t in FAR:
            if nxt == "*":
                out[i] = "" if prev == "(" else attr(t)
            elif prev == "*":
                out[i] = "" if depth > 0 else attr(t)
            elif nxtk == "id" and nxt2 == "(":
                out[i] = ""
            elif nxtk == "id":
                out[i] = "" if depth > 0 else attr(t)
            else:
                out[i] = ""
    return "".join(out)


# ---- inline assembly -------------------------------------------------------------------------------------------------------
# Borland's `__asm { ... }`, `__asm op`, `_asm` and `__emit__( bytes )` statements, assembled here as Open Watcom's front end does
# and handed on as a call of `llrm.ia16.code.<hex>.<offset>p<addend>`: each C variable the block names is a frame place passed
# by address, and the word that takes its displacement is patched by the backend. Statements that follow each other are one
# block (registers flow between them, and a call between them would lose that); a block that ends a function with a result is
# its `return`, since dx:ax is where the result is left.
import subprocess, tempfile, os
MC = os.environ.get("LLVM_MC", "/opt/homebrew/opt/llvm/bin/llvm-mc")
OBJCOPY = "/opt/homebrew/opt/llvm/bin/llvm-objcopy"
REGS = set("""ax bx cx dx si di bp sp ip al ah bl bh cl ch dl dh eax ebx ecx edx esi edi ebp esp cs ds es ss fs gs st cr0 cr2 cr3 cr4
              mm0 mm1 mm2 mm3 mm4 mm5 mm6 mm7 xmm0 xmm1 xmm2 xmm3 xmm4 xmm5 xmm6 xmm7""".split())
WORDS = set("ptr byte word dword qword tbyte near far short offset seg flat".split())
SIZES = [("double", "qword"), ("float", "dword"), ("long", "dword"), ("short", "word"), ("char", "byte"), ("int", "word"), ("unsigned", "word"), ("signed", "word")]
ASM_STMT = re.compile(r'(?:__asm|_asm|\basm)\b\s*(?:\{(?P<block>[^{}]*)\}|(?P<line>[^\n;{}]+))\s*;?|__emit__\s*\((?P<emit>\s*(?:0x[0-9a-fA-F]+|\d+)(?:\s*,\s*(?:0x[0-9a-fA-F]+|\d+))*\s*)\)\s*;?', re.S)

def c_size(text, name):
    m = None
    for m in re.finditer(r'\b((?:unsigned\s+|signed\s+|const\s+|static\s+|register\s+)*(?:double|float|long|short|int|char|unsigned|signed)(?:\s+(?:int|long|char|short))?)\b[^;(){}=]*?\b' + re.escape(name) + r'\b\s*[,;=\[)]', text):
        pass
    if not m: return None
    t = m.group(1)
    for key, size in SIZES:
        if re.search(r'\b' + key + r'\b', t):
            return "dword" if "long" in t else size
    return None

def assemble(lines, variables):
    """(bytes, [name in the order its displacement appears]) of the block, variables replaced by [bp+placeholder]."""
    order, body = [], []
    for line in lines:
        out, first = [], True
        toks = re.findall(r'\s+|[A-Za-z_]\w*|0x[0-9a-fA-F]+|\d+[hH]?|.', line)
        for n, t in enumerate(toks):
            if re.match(r'[A-Za-z_]\w*$', t) and not first and t.lower() not in REGS and t.lower() not in WORDS and t in variables:
                if t not in order: order.append(t)
                size = variables[t]
                prev = next((o.strip() for o in reversed(out) if o.strip()), "")
                holder = f"[bp+{0x7001 + order.index(t)}]"
                if prev.lower() != "ptr" and size: holder = f"{size} ptr {holder}"
                out.append(holder)
            else:
                out.append(t)
            if t.strip(): first = False
        body.append("".join(out))
    src = ".intel_syntax noprefix\n.code16\n" + "\n".join(body) + "\n"
    with tempfile.TemporaryDirectory() as d:
        open(f"{d}/a.s", "w").write(src)
        r = subprocess.run([MC, "-triple=i386-unknown-none", "-filetype=obj", f"{d}/a.s", "-o", f"{d}/a.o"], capture_output=True, text=True)
        if r.returncode: raise ValueError("asm: " + r.stderr.strip().splitlines()[0][:100] + " in " + src.replace("\n", " ; ")[:100])
        subprocess.run([OBJCOPY, "-O", "binary", "--only-section=.text", f"{d}/a.o", f"{d}/a.bin"], check=True)
        return open(f"{d}/a.bin", "rb").read(), order

def inline_asm(text):
    decls, count = [], [0]
    def merged():
        pass
    mask = re.sub(r'"(?:\\.|[^"\\])*"|\'(?:\\.|[^\'\\])*\'', lambda m: m.group()[0] + ' ' * (len(m.group()) - 2) + m.group()[-1], text)
    matches = list(ASM_STMT.finditer(mask))
    groups, cur = [], []
    for m in matches:
        if cur and text[cur[-1].end():m.start()].strip() == "": cur.append(m)
        else:
            if cur: groups.append(cur)
            cur = [m]
    if cur: groups.append(cur)
    pieces, last = [], 0
    for g in groups:
        start, end = g[0].start(), g[-1].end()
        lines = []
        for m in g:
            if m.group("emit") is not None:
                lines.append(".byte " + m.group("emit"))
            elif m.group("block") is not None:
                lines += [l.strip() for l in re.split(r'[\n;]', m.group("block")) if l.strip()]
            else:
                lines.append(m.group("line").strip())
        before = text[max(0, start - 6000):start]
        names = {t for l in lines for t in re.findall(r'[A-Za-z_]\w*', l)}
        variables = {n: c_size(before, n) for n in names if n.lower() not in REGS and n.lower() not in WORDS and c_size(before, n) is not None}
        # the first word of a line is the mnemonic, never a variable
        for l in lines:
            w = re.match(r'\s*([A-Za-z_]\w*)', l)
            if w and w.group(1) in variables and not re.search(r'\b' + w.group(1) + r'\b', l[w.end():]): variables.pop(w.group(1))
        data, order = assemble(lines, variables)
        places = []
        for k, name in enumerate(order):
            want = (0x7001 + k).to_bytes(2, "little")
            at = data.find(want)
            if at < 0: raise ValueError(f"asm: no displacement for {name}")
            places.append(f".{at}p0")
        sym = "llrm.ia16.code." + data.hex() + "".join(places)
        count[0] += 1
        fn = f"__llrm_asm{count[0]}"
        decls.append(f'extern long {fn}({", ".join(["void *"] * len(order)) or "void"}) __asm__("{sym}");')
        call = f"{fn}({', '.join('&' + n for n in order)})"
        after = text[end:end + 40].lstrip()
        ret = None
        if after.startswith("}"):
            hdr = None
            for h in re.finditer(r'(?:^|[;}\n])\s*((?:static\s+|extern\s+|inline\s+)*[A-Za-z_][\w\s\*]*?)\s+\w+\s*\([^;{}]*\)\s*\{', text[:start]):
                hdr = h
            if hdr:
                rt = re.sub(r'\b(static|extern|inline|near|far|pascal|cdecl|interrupt|_FAR|_NEAR)\b', '', hdr.group(1)).strip()
                if rt and rt != "void": ret = rt
        stmt = (f"return ({ret}) {call};" if ret else f"{call};")
        pieces.append(text[last:start]); pieces.append(stmt); last = end
    pieces.append(text[last:])
    return ("\n".join(decls) + "\n" if decls else "") + "".join(pieces)
