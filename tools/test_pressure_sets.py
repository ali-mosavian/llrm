"""pressure_sets reads the frame pointer's views from the description and the pin's register file."""

import unittest

import pressure_sets


class FrameViews(unittest.TestCase):
    def test_the_frame_register_and_its_views_but_the_byte_ones(self):
        """FRAME was a copied list of x86 names (target_facts counted 3); the description classes the frame register."""
        regs = ["# name bits root lane classes", "@scratch ax", "ebp 32 ebp 0 reserved,frame,base", "eax 32 eax 0 gpr"]

        def reg(*subs):
            return {"!superclasses": ["Register"], "SubRegs": [{"def": one} for one in subs]}

        pin = {"RBP": reg("EBP"), "EBP": reg("BP", "HBP"), "BP": reg("BPL", "BPH"), "BPL": reg(), "BPH": reg(), "HBP": reg(), "EAX": reg("AX")}
        self.assertEqual(pressure_sets.frame_views(regs, pin), {"RBP", "EBP", "BP"})


if __name__ == "__main__":
    unittest.main()
