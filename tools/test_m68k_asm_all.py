"""The assemble-everything sweep names every cell: gas refusing our assembly is its own status and fails the run."""
import sys
import unittest

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import m68k_asm_all as aa  # noqa: E402


class AsmAll(unittest.TestCase):
    def test_gas_refusing_our_assembly_is_an_asm_error_with_its_line(self):
        """68010 nbody-like cells printed `cmpi.l #7,(dst,%pc)`; the sweep must say ASM-ERROR and why, not drop the row."""
        status = aa.status_of(0, "", 1, "x.s: Assembler messages:\nx.s:46: Error: operands mismatch -- statement `cmpi.l #7,(dst,%pc)' ignored\n")
        self.assertTrue(status.startswith("ASM-ERROR operands mismatch"), status)

    def test_a_refusal_is_named_and_is_not_a_failure(self):
        status = aa.status_of(1, "llrm-m68k-llc: @five: an object aligned to 16 needs stack realignment, not supported yet (accepts 8)\n", None, "")
        self.assertTrue(status.startswith("refused"), status)
        self.assertEqual(aa.exit_status([status, "ok"]), 0)

    def test_the_exit_status_orders_asm_errors_above_compiler_failures(self):
        self.assertEqual(aa.exit_status(["ok", "ASM-ERROR x"]), 3)
        self.assertEqual(aa.exit_status(["ok", "cc-fail x"]), 2)
        self.assertEqual(aa.exit_status(["ASM-ERROR x", "cc-fail y"]), 3)

    def test_the_default_cpus_are_the_matrixs(self):
        self.assertIn("68010", aa.mx.CPUS)
        self.assertIn("68030", aa.mx.CPUS)


if __name__ == "__main__":
    unittest.main()
