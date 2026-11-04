"""x86 alignment padding of a code segment: the decoder of nop-family instructions that an assembler's `.p2align` writes. Target-owned (x86), so the
shared tools read it from here and hold no x86 decoder of their own (tests/target_facts.rs)."""


def is_pad(one) -> bool:
    """An instruction that does nothing and is what an assembler's `.p2align` writes: a nop of any length, or `mov`, `xchg` or `lea` of a register
    onto itself (`mov esi,esi`, `lea esi,[esi+0]`)."""
    from iced_x86 import Mnemonic, OpKind

    if one.mnemonic == Mnemonic.NOP:
        return True
    if one.op_count != 2 or one.op0_kind != OpKind.REGISTER:
        return False
    if one.mnemonic in (Mnemonic.MOV, Mnemonic.XCHG):
        return one.op1_kind == OpKind.REGISTER and one.op0_register == one.op1_register
    if one.mnemonic == Mnemonic.LEA:
        from iced_x86 import Register

        return one.op1_kind == OpKind.MEMORY and one.memory_base == one.op0_register and one.memory_index == Register.NONE and one.memory_displacement == 0
    return False


def padding(content: bytes, bits: int) -> tuple[int, int]:
    """(bytes of `content` that are alignment padding, bytes that decode to nothing) of a code segment's, or one function's, bytes. Padding is a
    maximal run of padding instructions (is_pad) that ends on a 2^k boundary, k >= 2, and is shorter than 2^k. A byte that starts no instruction (data
    inside code, the BASIC module header ahead of the code) is skipped and counted: the decoder takes up again at the next byte."""
    from iced_x86 import Code, Decoder

    total = skipped = run = 0
    decoder, at = Decoder(bits, content, ip=0), 0
    while at < len(content):
        decoder.position, decoder.ip = at, at
        one = decoder.decode()
        if one.code == Code.INVALID:
            total += pad_of(run, at)
            run = 0
            skipped += 1
            at += 1
            continue
        at += one.len
        if is_pad(one):
            run += one.len
        else:
            total += pad_of(run, one.ip)
            run = 0
    return total + pad_of(run, at), skipped


def pad_of(run: int, end: int) -> int:
    """`run` bytes of padding instructions that end at `end`: padding where `end` is a multiple of some 2^k > run, k >= 2; real code otherwise."""
    return run if run and end % 4 == 0 and run < (end & -end) else 0
