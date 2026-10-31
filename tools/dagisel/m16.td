// What real mode adds to the pin's x86 selector: patterns for the instructions the pin has and
// never selects. The pin's X86.td is included whole; every record below is ours (SD9).
include "X86.td"

// `les`/`lds` load a far pointer: the offset in a general register, the segment in ES/DS.
// The node's one result is the offset; the segment is the instruction's implicit def, read
// back with a CopyFromReg on the chain.
def SDT_X86FarLoad : SDTypeProfile<1, 1, [SDTCisVT<0, i16>, SDTCisPtrTy<1>]>;
def X86les : SDNode<"X86ISD::LES", SDT_X86FarLoad,
                    [SDNPHasChain, SDNPMayLoad, SDNPMemOperand]>;
def X86lds : SDNode<"X86ISD::LDS", SDT_X86FarLoad,
                    [SDNPHasChain, SDNPMayLoad, SDNPMemOperand]>;

let Predicates = [Not64BitMode] in {
def : Pat<(X86les addr:$src), (LES16rm addr:$src)>;
def : Pat<(X86lds addr:$src), (LDS16rm addr:$src)>;
}

// `rep movs` and `rep stos` on the 16-bit registers: the pin has SI/DI/CX only as ESI/EDI/ECX
// (with an address-size prefix) and RSI/RDI/RCX. In real mode the string instructions use
// DS:SI and ES:DI; the segments are set by the code around the node.
let SchedRW = [WriteMicrocoded] in {
let Defs = [CX, DI, SI], Uses = [CX, DI, SI], isCodeGenOnly = 1 in {
def REP_MOVSB_16 : I<0xA4, RawFrm, (outs), (ins),
                     "{rep;movsb (%si), %es:(%di)|rep movsb es:[di], [si]}",
                     []>, REP, AdSize16, Requires<[In16BitMode]>;
def REP_MOVSW_16 : I<0xA5, RawFrm, (outs), (ins),
                     "{rep;movsw (%si), %es:(%di)|rep movsw es:[di], [si]}",
                     []>, REP, AdSize16, OpSize16, Requires<[In16BitMode]>;
def REP_MOVSD_16 : I<0xA5, RawFrm, (outs), (ins),
                     "{rep;movsl (%si), %es:(%di)|rep movsd es:[di], [si]}",
                     []>, REP, AdSize16, OpSize32, Requires<[In16BitMode]>;
}

let Defs = [CX, DI], isCodeGenOnly = 1 in {
  let Uses = [AL, CX, DI] in
  def REP_STOSB_16 : I<0xAA, RawFrm, (outs), (ins),
                       "{rep;stosb %al, %es:(%di)|rep stosb es:[di], al}",
                       []>, REP, AdSize16, Requires<[In16BitMode]>;
  let Uses = [AX, CX, DI] in
  def REP_STOSW_16 : I<0xAB, RawFrm, (outs), (ins),
                       "{rep;stosw %ax, %es:(%di)|rep stosw es:[di], ax}",
                       []>, REP, AdSize16, OpSize16, Requires<[In16BitMode]>;
  let Uses = [EAX, CX, DI] in
  def REP_STOSD_16 : I<0xAB, RawFrm, (outs), (ins),
                       "{rep;stosl %eax, %es:(%di)|rep stosd es:[di], eax}",
                       []>, REP, AdSize16, OpSize32, Requires<[In16BitMode]>;
}
} // SchedRW

// Tried before the pin's 32-bit forms, which the same node also matches.
let AddedComplexity = 10, Predicates = [In16BitMode] in {
def : Pat<(X86rep_movs i8), (REP_MOVSB_16)>;
def : Pat<(X86rep_movs i16), (REP_MOVSW_16)>;
def : Pat<(X86rep_movs i32), (REP_MOVSD_16)>;
def : Pat<(X86rep_stos i8), (REP_STOSB_16)>;
def : Pat<(X86rep_stos i16), (REP_STOSW_16)>;
def : Pat<(X86rep_stos i32), (REP_STOSD_16)>;
}

// `lea` of a frame object or an address sum: the pin's `lea32addr` is typed i32 and the
// 16-bit `LEA16r` has no pattern, but real mode makes both i16.
def lea16addr : ComplexPattern<i16, 5, "selectLEAAddr",
                               [add, sub, mul, X86mul_imm, shl, or, xor, frameindex], []>;
let Predicates = [Not64BitMode] in
def : Pat<(i16 lea16addr:$src), (LEA16r lea16addr:$src)>;

// A wrapped symbol is a 16-bit immediate with a relocation (the pin has it for i32 and i64).
def : Pat<(i16 relocImm:$src), (MOV16ri relocImm:$src)>;

// `X86mul_imm` (a multiply by 3, 5 or 9 the combiner leaves for `lea`) when `lea` cannot take it,
// which is every real-mode scaled index: the multiply instruction.
let Predicates = [Not64BitMode] in {
def : Pat<(X86mul_imm GR16:$src1, imm:$src2), (IMUL16rri GR16:$src1, imm:$src2)>;
def : Pat<(X86mul_imm GR32:$src1, imm:$src2), (IMUL32rri GR32:$src1, imm:$src2)>;
}

// SD331: a byte widened to a word is the word form: in 16-bit mode the pin's `MOVZX32` of
// `sub_16bit` (X86InstrCompiler.td L1850) carries an operand-size prefix, +1 byte and +1 clock.
let AddedComplexity = 1, Predicates = [In16BitMode] in {
def : Pat<(i16 (zext GR8:$src)), (MOVZX16rr8 GR8:$src)>;
def : Pat<(zextloadi16i8 addr:$src), (MOVZX16rm8 addr:$src)>;
}
