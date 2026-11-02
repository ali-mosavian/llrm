#!/usr/bin/env python3
"""Check src/timing/z80.csv against Zilog's UM0080 (the Z80 CPU User Manual), the third and authoritative source.

    manual_check.py UM0080.pdf     (pdftotext -layout is run on it; the PDF is not in the repo)

Each CSV form is mapped to the manual's instruction section; its T-states (and the second timing of a conditional
form) must be among the section's `M Cycles / T States` rows. Labelled rows (`AND (HL)`, `SLA r`) are matched by label.
Forms with no row in a Z80 manual (the Z180's `mlt`) are listed, not passed. Exit 1 on any disagreement.
"""
import csv, os, re, subprocess, sys

HERE = os.path.dirname(os.path.abspath(__file__))
CSV = os.path.join(HERE, "..", "..", "src", "timing", "z80.csv")

# CSV form -> (manual section title, row label or None for the section's own rows)
SECTION = {
    "ld b;c": "LD r, r'", "ld b;n": "LD r,n", "ld b;(hl)": "LD r, (HL)", "ld (hl);b": "LD (HL), r",
    "ld (hl);n": "LD (HL), n", "ld a;(bc)": "LD A, (BC)", "ld a;(de)": "LD A, (DE)", "ld (bc);a": "LD (BC), A",
    "ld a;(nn)": "LD A, (nn)", "ld (nn);a": "LD (nn), A", "ld hl;(nn)": "LD HL, (nn)", "ld (nn);hl": "LD (nn), HL",
    "ld bc;(nn)": "LD dd, (nn)", "ld bc;nn": "LD dd, nn", "ld hl;nn": "LD dd, nn", "ld ix;nn": "LD IX, nn",
    "ld sp;hl": "LD SP, HL", "ld b;(ix+d)": "LD r, (IX+d)", "ld (ix+d);b": "LD (IX+d), r",
    "ld (ix+d);n": "LD (IX+d), n", "push bc": "PUSH qq", "pop bc": "POP qq", "push ix": "PUSH IX", "pop ix": "POP IX",
    "ex de;hl": "EX DE, HL", "ex (sp);hl": "EX (SP), HL", "add a;b": "ADD A, r", "add a;n": "ADD A, n",
    "add a;(hl)": "ADD A, (HL)", "add a;(ix+d)": "ADD A, (IX + d)", "inc b": "INC r", "dec b": "DEC m@DEC r",
    "inc (hl)": "INC (HL)", "dec (hl)": "DEC m@DEC (HL)", "inc (ix+d)": "INC (IX+d)", "inc bc": "INC ss",
    "dec bc": "DEC ss", "inc ix": "INC IX", "inc hl": "INC ss", "add hl;bc": "ADD HL, ss", "adc hl;bc": "ADC HL, ss",
    "sbc hl;bc": "SBC HL, ss", "add ix;bc": "ADD IX, pp", "rlca": "RLCA", "rrca": "RRCA", "rla": "RLA", "rra": "RRA",
    "cpl": "CPL", "neg": "NEG", "scf": "SCF", "ccf": "CCF", "nop": "NOP", "bit 0;b": "BIT b, r",
    "bit 0;(hl)": "BIT b, (HL)", "set 0;b": "SET b, r", "res 0;b": "RES b, m@RES r", "djnz e": "DJNZ, e",
    "jp nn": "JP nn", "jp nz;nn": "JP cc, nn", "jp (hl)": "JP (HL)", "jr e": "JR e", "jr nz;e": "JR NZ, e",
    "jr c;e": "JR C, e", "jr nc;e": "JR NC, e", "call nn": "CALL nn", "call nz;nn": "CALL cc, nn", "ret": "RET",
    "ret nz": "RET cc", "ldir": "LDIR", "cpir": "CPIR", "rlc b": "RLC r",
    "adc a;b": "ADC A, s@ADC A, r", "sub b": "SUB s@SUB r", "sub n": "SUB s@SUB n", "sbc a;b": "SBC A, s@SBC A, r",
    "and b": "AND s@AND r", "and n": "AND s@AND n", "or b": "OR s@OR r", "or n": "OR s@OR n", "xor b": "XOR s@XOR r",
    "xor n": "XOR s@XOR n", "cp b": "CP s@CP r", "cp n": "CP s@CP n", "cp (hl)": "CP s@CP (HL)",
    "sla b": "SLA m@SLA r", "sra b": "SRA m@SRA r", "srl b": "SRL m@SRL r", "rl b": "RL m@RL r", "rr b": "RR m@RR r",
    "sla (hl)": "SLA m@SLA (HL)",
}


def sections(text):
    lines = text.split("\n")
    out = {}
    for i, line in enumerate(lines):
        if line.strip() != "Operation" or i == 0:
            continue
        k = i - 1
        while k > 0 and not lines[k].strip():
            k -= 1
        end = min(len(lines), i + 140)
        for m in range(i + 1, end):
            if lines[m].strip() == "Operation":
                end = m
                break
        out[lines[k].strip()] = [" ".join(x.split()) for x in lines[i:end]]
    return out


def rows(body):
    found = []
    for line in body:
        if "MHz" in line:
            continue
        m = re.match(r"^(.*?)\s*\b(\d)\s+(\d{1,2})\s*(?:\(|\d\.\d)", line)
        if m:
            found.append((m.group(1).replace("lY", "IY"), int(m.group(3))))
    return found


def main():
    text = subprocess.run(["pdftotext", "-layout", sys.argv[1], "-"], capture_output=True, text=True, check=True).stdout
    secs = sections(text)
    bad, ok, absent = 0, 0, []
    for form, nbytes, taken, not_taken, cpu in csv.reader(l for l in open(CSV) if not l.startswith("#")):
        key = SECTION.get(form)
        if key is None:
            absent.append(form)
            continue
        title, _, label = key.partition("@")
        body = rows(secs.get(title, []))
        if label:
            body = [(l, t) for l, t in body if l == label]
        have = [t for _, t in body]
        want = [int(taken)] + ([int(not_taken)] if not_taken else [])
        if all(w in have for w in want):
            ok += 1
        else:
            print(f"MANUAL {form}: csv {want}, manual `{title}` {label or ''} {have}")
            bad += 1
    print(f"{ok} forms agree with the manual, {bad} disagree; not in a Z80 manual: {absent}")
    return 1 if bad else 0


sys.exit(main())
