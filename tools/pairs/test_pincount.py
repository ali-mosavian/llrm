"""pincount counts instructions the same way on both sides: directives, labels and comments are none."""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import pincount  # noqa: E402


def test_pin_counts_only_tabbed_instructions():
    asm = "\t.text\n_f:\n\tpushl\t%ebp\n\tmovl\t%esp, %ebp\n.LBB0_1:\n\t.p2align\t4\n\tretl\n"
    assert pincount.count_pin(asm) == 3


def test_ours_counts_indented_lines_that_are_not_directives_or_comments():
    asm = "_f proc near\n    push ebp\n    align 16\n    ; note\n    db 1,2\n    mov ebp, esp\nL0_1:\n    ret\n"
    assert pincount.count_ours(asm) == 3
