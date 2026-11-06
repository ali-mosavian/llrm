import sys
from pathlib import Path

from iced_x86 import Decoder
from unicorn import UC_ARCH_X86, UC_MODE_16, Uc
from unicorn.x86_const import UC_X86_REG_BX, UC_X86_REG_DS

sys.path.insert(0, str(Path(__file__).parent))
import icount  # noqa: E402


def multiplier_of_imul_through_ds_bx(word):
    """`imul ax, word ptr [bx]` with DS=0x100, BX=0x10 and `word` at DS:BX."""
    uc = Uc(UC_ARCH_X86, UC_MODE_16)
    uc.mem_map(0, 0x100000)
    uc.mem_write(0x1010, word.to_bytes(2, "little") + b"\xff\xff")  # the next word is set: the reader takes the operand's width, not 4 bytes
    uc.reg_write(UC_X86_REG_DS, 0x100)
    uc.reg_write(UC_X86_REG_BX, 0x10)
    one = Decoder(16, bytes([0x0F, 0xAF, 0x07])).decode()
    price, multiplier_of, _ = icount._prices()
    factor = icount._multiplier(multiplier_of, uc, one)
    return factor, price(one, False, True, 1, factor, 0)


def test_a_16_bit_multiply_by_a_memory_word_is_priced_by_the_words_value_not_the_tables_least():
    """Every `imul r16, [bx]` of a 16-bit program raised in vsgcc's flat reader and fell back to None (the table's least): the m16 clock of a program
    that multiplies through memory did not depend on the multiplier."""
    small, cheap = multiplier_of_imul_through_ds_bx(3)
    large, dear = multiplier_of_imul_through_ds_bx(0x7FFF)
    assert (small, large) == (3, 0x7FFF)
    assert cheap < dear
