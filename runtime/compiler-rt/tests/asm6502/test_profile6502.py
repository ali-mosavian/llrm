#!/usr/bin/env python3
"""A hand-assembled program with a hand-derived cycle count: indexed page crossing, taken and crossing branches, RMW, JSR/RTS,
and the free entry into the exit hook. Also run on sim65 when it is installed."""
import os, shutil, subprocess, sys, tempfile, unittest
import profile6502 as P

# (bytes, cycles)
PROG = [
    ([0xA2, 0x03], 2),              # 0200 ldx #3
    ([0xA9, 0x00], 2),              # 0202 lda #0
    ([0x18], 2), ([0x69, 0x01], 2), ([0xCA], 2),   # 0204 loop: clc / adc #1 / dex, three times
    ([0xD0, 0xFA], None),           # 0208 bne loop: 3 + 3 + 2
    ([0xA2, 0x01], 2),              # ldx #1
    ([0xBD, 0xFF, 0x20], 5),        # lda $20FF,x: crosses a page, 4 + 1
    ([0xA0, 0x00], 2),              # ldy #0
    ([0x99, 0x00, 0x30], 5),        # sta $3000,y: stores pay no penalty
    ([0xFE, 0x00, 0x30], 7),        # inc $3000,x
    ([0x20, 0x10, 0x03], 6),        # jsr $0310
    ([0x4C, 0xFE, 0x02], 3),        # jmp $02fe
]
LOOP = 3 * 6 + (3 + 3 + 2)
EXPECT = 2 + 2 + LOOP + 2 + 5 + 2 + 5 + 7 + 6 + (2 + 6) + 3 + 4   # sub = lda #7 / rts; the bne at $02FE to $0302 costs 4
def build():
    mem = bytearray(0x400)
    code = bytearray()
    for b, _ in PROG: code += bytes(b)
    mem[0:len(code)] = code
    mem[0xFE:0x100] = bytes([0xD0, 0x02])       # $02FE bne $0302: Z is clear after lda #7; sim65 charges 4 (opcode page differs from the target's)
    mem[0x102:0x105] = bytes([0x4C, 0xF9, 0xFF])  # $0302 jmp exit
    mem[0x110:0x113] = bytes([0xA9, 0x07, 0x60])  # $0310 lda #7 / rts
    return bytes([*b'sim65', 2, 0, 0x00, 0x00, 0x02, 0x00, 0x02]) + bytes(mem)

class T(unittest.TestCase):
    def test_cycles(self):
        r = P.run(build())
        self.assertEqual((r.cycles, r.exit), (EXPECT, 7))
    def test_sim65_agrees(self):
        if not shutil.which('sim65'): self.skipTest('no sim65')
        with tempfile.NamedTemporaryFile(suffix='.bin') as f:
            f.write(build()); f.flush()
            p = subprocess.run(['sim65', '-c', f.name], capture_output=True, text=True)
        self.assertEqual((int(p.stdout.split()[0]), p.returncode), (EXPECT, 7))
    def test_wrong_table_entry_is_caught(self):
        """lda abs,x at 4 + 1 page cycle: a table entry of 3 must change the total."""
        name, mode, cyc, cls = P._T[0xBD]
        P._T[0xBD] = (name, mode, 3, cls)
        try: r = P.run(build())
        finally: P._T[0xBD] = (name, mode, cyc, cls)
        self.assertNotEqual(r.cycles, EXPECT)
if __name__ == '__main__': unittest.main()
