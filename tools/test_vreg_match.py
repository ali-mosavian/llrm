"""Toy pair of dumps: same function, different vreg numbers, one differing decision."""
import os
import sys
import unittest

sys.path.insert(0, os.path.dirname(__file__))
import vreg_match as vm

OURS_IN = """\
IN b0[0] #1 d[10] u[] gNone Some((Some("mov"), [], []))
IN b0[1] #2 d[11] u[10] gNone Some((Some("add"), [], []))
IN b0[2] #3 d[12] u[11, 10] gNone Some((Some("imul"), [], []))
IN b0[3] #4 d[] u[12, 12] gNone Some((Some("test"), [], []))
"""
OURS_DEC = """\
== f
dec v10 stage=Assign size=3 w=5.0 -> assign ESI
dec v11 stage=Assign size=2 w=4.0 -> assign EDI
dec v12 stage=Assign size=2 w=3.0 -> assign EAX
"""
PIN_MIR = """\
bb.0:
  %3:gr32 = MOV32ri 7
  %4:gr32 = ADD32ri %3, 1, implicit-def $eflags
  %5:gr32 = IMUL32rr %4, %3, implicit-def $eflags
  TEST32rr %5, %5, implicit-def $eflags
"""
PIN_RA = """\
********** GREEDY REGISTER ALLOCATION **********
********** Function: g
selectOrSplit GR32:%9 [0r,8r:0) 0@0r  weight:1.0
assigning %9 to $ecx: CL [0r,8r:0)
********** GREEDY REGISTER ALLOCATION **********
********** Function: f
selectOrSplit GR32:%3 [0r,8r:0) 0@0r  weight:5.0
assigning %3 to $esi: SIL [0r,8r:0)
selectOrSplit GR32:%4 [0r,8r:0) 0@0r  weight:4.0
assigning %4 to $r8d: R8B [0r,8r:0)
selectOrSplit GR32:%5 [0r,8r:0) 0@0r  weight:3.0
assigning %5 to $eax: AL [0r,8r:0)
"""


class VregMatch(unittest.TestCase):
    def test_vregs_match_by_defining_instruction(self):
        m, agreement = vm.match(vm.ours_stream(OURS_IN), vm.pin_stream(PIN_MIR))
        self.assertEqual(m, {10: 3, 11: 4, 12: 5})
        self.assertEqual(agreement, 1.0)

    def test_condition_codes_and_live_in_headers(self):
        self.assertEqual(vm.mnemonic("jne"), vm.mnemonic("JCC_1"))
        self.assertEqual(vm.mnemonic("sete"), vm.mnemonic("SETCCr"))
        self.assertEqual(vm.mnemonic("jmp"), vm.mnemonic("JMP_1"))
        self.assertEqual(vm.pin_stream("Function Live Ins: $esi in %427, $edx in %428\n"), [])

    def test_register_names_compare_across_widths(self):
        self.assertEqual(vm.reg_key("EDX"), vm.reg_key("$rdx"))
        self.assertEqual(vm.reg_key("R8D"), vm.reg_key("$r8d"))
        self.assertNotEqual(vm.reg_key("ESI"), vm.reg_key("$edi"))

    def test_first_differing_decision_is_named_in_pin_order(self):
        out = vm.report(OURS_IN, OURS_DEC, PIN_MIR, PIN_RA, "f")
        self.assertIn("pin %4 -> r8   ours v11 -> di", out)
        self.assertIn("same 2  differ 1", out)

    def test_our_log_is_cut_at_the_function_header(self):
        log = "== g\ndec v10 stage=Assign size=1 w=1 -> assign ECX\n" + OURS_DEC
        self.assertEqual(vm.ours_decisions(log, "f")[0], (10, "si"))

    def test_one_pin_vreg_that_is_several_of_ours_is_not_a_register_difference(self):
        ours_in = OURS_IN.replace("IN b0[3]", 'IN b0[3] #5 d[13] u[] gNone Some((Some("mov"), [], []))\nIN b0[4]')
        pin_mir = PIN_MIR.replace("  TEST", "  %3:gr32 = MOV32ri 9\n  TEST")
        dec = OURS_DEC + "dec v13 stage=Assign size=1 w=1 -> assign ECX\n"
        out = vm.report(ours_in, dec, pin_mir, PIN_RA, "f")
        self.assertIn("pin %3 -> si   ours has 2 vregs", out)
        self.assertIn("pin vreg = several of ours 1", out)

    def test_only_the_named_function_is_read(self):
        self.assertEqual([p for p, _ in vm.pin_decisions(PIN_RA, "g")], [9])


if __name__ == "__main__":
    unittest.main()
