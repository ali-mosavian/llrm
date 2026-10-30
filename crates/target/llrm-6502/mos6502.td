// Toy 6502 target for the DAG-selector spike: i8 in zero-page "imaginary"
// registers (llvm-mos convention), i16 pointers as register pairs, carry as
// the flags register P. Shaped after LLVM's AVR target.

include "llvm/Target/Target.td"

def sub_lo : SubRegIndex<8> { let Namespace = "MOS"; }
def sub_hi : SubRegIndex<8, 8> { let Namespace = "MOS"; }

class MOSReg<string n> : Register<n> { let Namespace = "MOS"; }
class MOSPair<string n, list<Register> parts> : RegisterWithSubRegs<n, parts> {
  let Namespace = "MOS";
  let SubRegIndices = [sub_lo, sub_hi];
}

def A : MOSReg<"a">;
def X : MOSReg<"x">;
def Y : MOSReg<"y">;
foreach i = 0-7 in def RC#i : MOSReg<"rc"#i>;
foreach i = 0-3 in def RS#i : MOSPair<"rs"#i, [!cast<Register>("RC"#!mul(i, 2)), !cast<Register>("RC"#!add(!mul(i, 2), 1))]>;
def P : MOSReg<"p">;
def C : MOSReg<"c">;

def Imag8 : RegisterClass<"MOS", [i8], 8, (sequence "RC%u", 0, 7)>;
// Every byte a copy may name: the three real registers and the zero-page bytes.
def Reg8 : RegisterClass<"MOS", [i8], 8, (add A, X, Y, Imag8)>;
def Imag16 : RegisterClass<"MOS", [i16], 8, (sequence "RS%u", 0, 3)>;
// The carry flag as an i1 value: one register, so the allocator sees the 6502's single carry.
def Carry : RegisterClass<"MOS", [i1], 8, (add C)>;
def Flags : RegisterClass<"MOS", [i8], 8, (add P)> { let CopyCost = -1; }

// 65C02 adds STZ, among others.
def HasCMOS : Predicate<"Subtarget->hasCMOS()">;

// (zp),Y addressing: a pointer in an Imag16 pair.
// The pair and a byte offset (the Y register): `(add ptr, 0..255)` or a bare pointer.
def memri : Operand<i16> { let MIOperandInfo = (ops Imag16, i8imm); }
def memrr : Operand<i16> { let MIOperandInfo = (ops Imag16, Imag8); }
def MOSAddr : ComplexPattern<i16, 2, "SelectIndirect", [], []>;
// (add pointer, (zext i8 index)): LDA (p),Y with the index in Y.
def MOSIdx : ComplexPattern<i16, 2, "SelectIndexed", [], []>;
// A pointer with no offset to add (and no frame object, which the frame code turns into an offset).
def MOSPtr : ComplexPattern<i16, 1, "SelectPlain", [], []>;

// The pin's tblgen has no nodes for the overflow forms the type legalizer expands into.
def SDTOverflowOp : SDTypeProfile<2, 2, [SDTCisSameAs<0, 2>, SDTCisSameAs<0, 3>, SDTCisInt<0>, SDTCisVT<1, i1>]>;
def SDTOverflowCarryOp : SDTypeProfile<2, 3, [SDTCisSameAs<0, 2>, SDTCisSameAs<0, 3>, SDTCisInt<0>, SDTCisVT<1, i1>, SDTCisVT<4, i1>]>;
def uaddo : SDNode<"ISD::UADDO", SDTOverflowOp, [SDNPCommutative]>;
def usubo : SDNode<"ISD::USUBO", SDTOverflowOp>;
def uaddo_carry : SDNode<"ISD::UADDO_CARRY", SDTOverflowCarryOp>;
def usubo_carry : SDNode<"ISD::USUBO_CARRY", SDTOverflowCarryOp>;

// Flags (the register P) as an i8 value, as m68k's condition-code register is. The condition code is a
// constant: EQ 0, NE 1, CC 2 (C clear: unsigned <), CS 3 (C set: unsigned >=), MI 4, PL 5.
def SDTMOSCmp : SDTypeProfile<1, 2, [SDTCisVT<0, i8>, SDTCisSameAs<1, 2>]>;
def SDTMOSBrcond : SDTypeProfile<0, 3, [SDTCisVT<0, OtherVT>, SDTCisVT<1, i8>, SDTCisVT<2, i8>]>;
def SDTMOSSetcc : SDTypeProfile<1, 2, [SDTCisVT<0, i8>, SDTCisVT<1, i8>, SDTCisVT<2, i8>]>;
def SDTMOSCmov : SDTypeProfile<1, 4, [SDTCisSameAs<0, 1>, SDTCisSameAs<1, 2>, SDTCisVT<3, i8>, SDTCisVT<4, i8>]>;
def MOScmp : SDNode<"MOSISD::CMP", SDTMOSCmp>;
// Equality: the Z flag of a - b over all the bytes (cmp's own Z covers the high byte only).
def MOScmpz : SDNode<"MOSISD::CMPZ", SDTMOSCmp>;
def MOSbrcond : SDNode<"MOSISD::BRCOND", SDTMOSBrcond, [SDNPHasChain]>;
def MOSsetcc : SDNode<"MOSISD::SETCC", SDTMOSSetcc>;
def MOScmov : SDNode<"MOSISD::CMOV", SDTMOSCmov>;

def SDT_MOSCallSeqStart : SDCallSeqStart<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def SDT_MOSCallSeqEnd : SDCallSeqEnd<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def MOScallseq_start : SDNode<"ISD::CALLSEQ_START", SDT_MOSCallSeqStart, [SDNPHasChain, SDNPOutGlue]>;
def MOScallseq_end : SDNode<"ISD::CALLSEQ_END", SDT_MOSCallSeqEnd, [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue]>;
def SDT_MOSCall : SDTypeProfile<0, -1, [SDTCisVT<0, i16>]>;
def MOScall : SDNode<"MOSISD::CALL", SDT_MOSCall, [SDNPHasChain, SDNPOutGlue, SDNPOptInGlue, SDNPVariadic]>;
def MOSwrapper : SDNode<"MOSISD::WRAPPER", SDTIntUnaryOp>;
def MOSretglue : SDNode<"MOSISD::RET_GLUE", SDTNone, [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;

class MOSInst<dag outs, dag ins, string asm, list<dag> pattern> : Instruction {
  let Namespace = "MOS";
  let OutOperandList = outs;
  let InOperandList = ins;
  let AsmString = asm;
  let Pattern = pattern;
}

// Byte and word arithmetic on imaginary registers. A word op is the two byte ops in a row, expanded when
// the instruction is emitted (llvm-mos does the same with its Imag16 pseudos). CLC; LDA a; ADC b; STA d.
multiclass Arith<string n, RegisterClass rc, ValueType vt> {
  let Defs = [P] in {
    def ADD#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "add $d, $a, $b", [(set vt:$d, (add vt:$a, vt:$b))]>;
    def SUB#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "sub $d, $a, $b", [(set vt:$d, (sub vt:$a, vt:$b))]>;
    def AND#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "and $d, $a, $b", [(set vt:$d, (and vt:$a, vt:$b))]>;
    def OR#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "or $d, $a, $b", [(set vt:$d, (or vt:$a, vt:$b))]>;
    def XOR#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "xor $d, $a, $b", [(set vt:$d, (xor vt:$a, vt:$b))]>;
  }
  // The carry is an operand, so these define no other flag (a third result type is not supported).
  // CLC; LDA a; ADC b; STA d: the carry out is the 6502's C.
    def ADDO#n : MOSInst<(outs rc:$d, Carry:$co), (ins rc:$a, rc:$b), "addo $d, $co, $a, $b",
                         [(set vt:$d, i1:$co, (uaddo vt:$a, vt:$b))]>;
    def SUBO#n : MOSInst<(outs rc:$d, Carry:$co), (ins rc:$a, rc:$b), "subo $d, $co, $a, $b",
                         [(set vt:$d, i1:$co, (usubo vt:$a, vt:$b))]>;
    // LDA a; ADC b; STA d, with the carry the previous instruction left in C.
    def ADDOC#n : MOSInst<(outs rc:$d, Carry:$co), (ins rc:$a, rc:$b, Carry:$ci), "addoc $d, $co, $a, $b, $ci",
                          [(set vt:$d, i1:$co, (uaddo_carry vt:$a, vt:$b, i1:$ci))]>;
    def SUBOC#n : MOSInst<(outs rc:$d, Carry:$co), (ins rc:$a, rc:$b, Carry:$ci), "suboc $d, $co, $a, $b, $ci",
                          [(set vt:$d, i1:$co, (usubo_carry vt:$a, vt:$b, i1:$ci))]>;
}
defm "" : Arith<"8", Imag8, i8>;
defm "" : Arith<"16", Imag16, i16>;
// x + 1 and x - 1 in place: INX/INY/INC on a cell, INC A on the 65C02 (CLC; ADC #1 otherwise). The allocator
// tells the register apart; the instruction is the same on every CPU.
let Defs = [P], AddedComplexity = 10 in {
  def INC8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a), "inc $d, $a", [(set i8:$d, (add i8:$a, (i8 1)))]>;
  def DEC8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a), "dec $d, $a", [(set i8:$d, (add i8:$a, (i8 -1)))]>;
}

// Shifts by a constant: one ASL/LSR/ROR step per bit.
multiclass Shift<string n, RegisterClass rc, ValueType vt> {
  let Defs = [P] in {
    def SHL#n : MOSInst<(outs rc:$d), (ins rc:$a, i8imm:$k), "shl $d, $a, $k", [(set vt:$d, (shl vt:$a, (i8 imm:$k)))]>;
    def SRL#n : MOSInst<(outs rc:$d), (ins rc:$a, i8imm:$k), "srl $d, $a, $k", [(set vt:$d, (srl vt:$a, (i8 imm:$k)))]>;
    def SRA#n : MOSInst<(outs rc:$d), (ins rc:$a, i8imm:$k), "sra $d, $a, $k", [(set vt:$d, (sra vt:$a, (i8 imm:$k)))]>;
  }
}
defm "" : Shift<"8", Imag8, i8>;
defm "" : Shift<"16", Imag16, i16>;

// Multiply, divide, remainder and shifts by a variable are calls of the runtime (runtime/mos_rt.s),
// expanded where the instruction is emitted: arguments in rc2.., the result in A (and X).
multiclass Runtime<string n, RegisterClass rc, ValueType vt> {
  let Defs = [P] in {
    def MUL#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "mul $d, $a, $b", [(set vt:$d, (mul vt:$a, vt:$b))]>;
    def UDIV#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "udiv $d, $a, $b", [(set vt:$d, (udiv vt:$a, vt:$b))]>;
    def SDIV#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "sdiv $d, $a, $b", [(set vt:$d, (sdiv vt:$a, vt:$b))]>;
    def UREM#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "urem $d, $a, $b", [(set vt:$d, (urem vt:$a, vt:$b))]>;
    def SREM#n : MOSInst<(outs rc:$d), (ins rc:$a, rc:$b), "srem $d, $a, $b", [(set vt:$d, (srem vt:$a, vt:$b))]>;
    def SHLV#n : MOSInst<(outs rc:$d), (ins rc:$a, Imag8:$b), "shlv $d, $a, $b", [(set vt:$d, (shl vt:$a, i8:$b))]>;
    def SRLV#n : MOSInst<(outs rc:$d), (ins rc:$a, Imag8:$b), "srlv $d, $a, $b", [(set vt:$d, (srl vt:$a, i8:$b))]>;
    def SRAV#n : MOSInst<(outs rc:$d), (ins rc:$a, Imag8:$b), "srav $d, $a, $b", [(set vt:$d, (sra vt:$a, i8:$b))]>;
  }
}
defm "" : Runtime<"8", Imag8, i8>;
defm "" : Runtime<"16", Imag16, i16>;

def LDI8 : MOSInst<(outs Imag8:$d), (ins i8imm:$v), "ldi $d, $v", [(set i8:$d, imm:$v)]>;
def LDI16 : MOSInst<(outs Imag16:$d), (ins i16imm:$v), "ldi $d, $v", [(set i16:$d, imm:$v)]>;

// LDY #off; LDA (p),Y; STA d. A word is two such, with INY between.
let mayLoad = 1 in {
  def LDP8 : MOSInst<(outs Imag8:$d), (ins memri:$p), "ldp $d, ($p)", [(set i8:$d, (load MOSAddr:$p))]>;
  def LDP16 : MOSInst<(outs Imag16:$d), (ins memri:$p), "ldp16 $d, ($p)", [(set i16:$d, (load MOSAddr:$p))]>;
}
let mayLoad = 1, AddedComplexity = 10 in
  def LDPI8 : MOSInst<(outs Imag8:$d), (ins memrr:$a), "ldpi $d, ($a)", [(set i8:$d, (load MOSIdx:$a))]>;
// A global indexed by a byte: LDX i; LDA g,X. The address is the symbol plus the zero-extended index.
def absx : Operand<i16> { let MIOperandInfo = (ops i16imm, Imag8); }
def MOSAbsX : ComplexPattern<i16, 2, "SelectAbsX", [], []>;
let mayLoad = 1, AddedComplexity = 10 in
  def LDAX : MOSInst<(outs Imag8:$d), (ins absx:$a), "ldax $d, $a", [(set i8:$d, (load MOSAbsX:$a))]>;
let mayStore = 1, AddedComplexity = 10 in
  def STAX : MOSInst<(outs), (ins Imag8:$s, absx:$a), "stax $s, $a", [(store i8:$s, MOSAbsX:$a)]>;
// A global plus a constant offset: LDA g+k.
def absaddr : Operand<i16> { let MIOperandInfo = (ops i16imm, i16imm); }
def MOSAbs : ComplexPattern<i16, 2, "SelectAbs", [], []>;
let mayLoad = 1, AddedComplexity = 20 in
  def LDABS : MOSInst<(outs Imag8:$d), (ins absaddr:$a), "ldabs $d, $a", [(set i8:$d, (load MOSAbs:$a))]>;
let mayStore = 1, AddedComplexity = 20 in
  def STABS : MOSInst<(outs), (ins Imag8:$s, absaddr:$a), "stabs $s, $a", [(store i8:$s, MOSAbs:$a)]>;
let mayStore = 1 in {
  def STP8 : MOSInst<(outs), (ins Imag8:$s, memri:$p), "stp $s, ($p)", [(store i8:$s, MOSAddr:$p)]>;
  def STP16 : MOSInst<(outs), (ins Imag16:$s, memri:$p), "stp16 $s, ($p)", [(store i16:$s, MOSAddr:$p)]>;
}
// 65C02 only: the unindexed (zp) mode, `lda (p)` / `sta (p)`, for an offset of 0.
let Predicates = [HasCMOS], AddedComplexity = 10 in {
  let mayLoad = 1 in
    def LDPZ : MOSInst<(outs Imag8:$d), (ins Imag16:$p), "ldpz $d, ($p)", [(set i8:$d, (load MOSPtr:$p))]>;
  let mayStore = 1 in
    def STPZ : MOSInst<(outs), (ins Imag8:$s, Imag16:$p), "stpz $s, ($p)", [(store i8:$s, MOSPtr:$p)]>;
  def : Pat<(i16 (zextloadi8 MOSPtr:$p)), (REG_SEQUENCE Imag16, (LDPZ MOSPtr:$p), sub_lo, (LDI8 0), sub_hi)>;
  def : Pat<(i16 (extloadi8 MOSPtr:$p)), (REG_SEQUENCE Imag16, (LDPZ MOSPtr:$p), sub_lo, (LDI8 0), sub_hi)>;
}
let mayStore = 1, AddedComplexity = 10 in
  def STPI8 : MOSInst<(outs), (ins Imag8:$s, memrr:$a), "stpi $s, ($a)", [(store i8:$s, MOSIdx:$a)]>;
// 65C02 only: the unindexed (zp) mode and absolute STZ for a store of zero. A zero constant is not a pattern here:
// it stays LDI8 and emission makes the one that lands in a cell `stz`, since a pattern would pin every zero to a cell.
let Predicates = [HasCMOS] in {
  let mayStore = 1 in
    def STZP : MOSInst<(outs), (ins memri:$p), "stzp ($p)", [(store (i8 0), MOSAddr:$p)]>;
  let mayStore = 1, AddedComplexity = 30 in
    def STZABS : MOSInst<(outs), (ins absaddr:$a), "stzabs $a", [(store (i8 0), MOSAbs:$a)]>;
}

def brtarget : Operand<OtherVT>;
def calltarget : Operand<i16>;

// Instructions the selector does not match from a pattern: copies, compare, branches, call. They exist
// so the emitter is keyed to generated opcode ids.
def COPY8 : MOSInst<(outs Reg8:$d), (ins Reg8:$s), "copy $d, $s", []>;
def COPY16 : MOSInst<(outs Imag16:$d), (ins Imag16:$s), "copy $d, $s", []>;
// LDA a; CMP b: the flags of a - b.
def CMP8 : MOSInst<(outs Flags:$f), (ins Imag8:$a, Imag8:$b), "cmp $f, $a, $b", [(set i8:$f, (MOScmp i8:$a, i8:$b))]>;
// LDA alo; CMP blo; LDA ahi; SBC bhi: C is a >= b unsigned and N the sign of a - b.
def CMP16 : MOSInst<(outs Flags:$f), (ins Imag16:$a, Imag16:$b), "cmp16 $f, $a, $b", [(set i8:$f, (MOScmp i16:$a, i16:$b))]>;
// Z only when both bytes are equal.
def CMPZ16 : MOSInst<(outs Flags:$f), (ins Imag16:$a, Imag16:$b), "cmpz16 $f, $a, $b", [(set i8:$f, (MOScmpz i16:$a, i16:$b))]>;
let isBranch = 1, isTerminator = 1 in {
  def BEQ : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "beq $t", [(MOSbrcond bb:$t, (i8 0), i8:$f)]>;
  def BNE : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bne $t", [(MOSbrcond bb:$t, (i8 1), i8:$f)]>;
  def BCC : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bcc $t", [(MOSbrcond bb:$t, (i8 2), i8:$f)]>;
  def BCS : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bcs $t", [(MOSbrcond bb:$t, (i8 3), i8:$f)]>;
  def BMI : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bmi $t", [(MOSbrcond bb:$t, (i8 4), i8:$f)]>;
  def BPL : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bpl $t", [(MOSbrcond bb:$t, (i8 5), i8:$f)]>;
}
def CMOV16 : MOSInst<(outs Imag16:$d), (ins Imag16:$f0, Imag16:$t, i8imm:$cc, Flags:$f), "cmov16 $d, $f0, $t, $cc, $f",
                     [(set i16:$d, (MOScmov i16:$f0, i16:$t, imm:$cc, i8:$f))]>;
// A condition as a byte 0 or 1, and a select: branches in the emitted code, flags saved around the loads.
def SETCC8 : MOSInst<(outs Imag8:$d), (ins i8imm:$cc, Flags:$f), "setcc $d, $cc, $f", [(set i8:$d, (MOSsetcc imm:$cc, i8:$f))]>;
def CMOV8 : MOSInst<(outs Imag8:$d), (ins Imag8:$f0, Imag8:$t, i8imm:$cc, Flags:$f), "cmov $d, $f0, $t, $cc, $f",
                    [(set i8:$d, (MOScmov i8:$f0, i8:$t, imm:$cc, i8:$f))]>;
let isBranch = 1, isTerminator = 1, isBarrier = 1 in
  def JMP : MOSInst<(outs), (ins brtarget:$t), "jmp $t", [(br bb:$t)]>;
let isBranch = 1, isTerminator = 1, isBarrier = 1, Predicates = [HasCMOS], AddedComplexity = 10 in
  def BRA : MOSInst<(outs), (ins brtarget:$t), "bra $t", [(br bb:$t)]>;
let isCall = 1 in {
  def JSR : MOSInst<(outs), (ins calltarget:$t), "jsr $t", [(MOScall tglobaladdr:$t)]>;
  // The 6502 has no indirect jsr: the return address and target-1 are pushed and an rts enters the callee.
  def JSRI : MOSInst<(outs), (ins Imag16:$p), "jsr ($p)", [(MOScall i16:$p)]>;
}
// a routine the selector names itself (the libcall of a fill or copy of unknown size)
def : Pat<(MOScall texternalsym:$t), (JSR texternalsym:$t)>;

// The call sequence's stack adjustment, resolved by the frame code once the frame is laid out.
let Defs = [RS0, P], Uses = [RS0] in {
  def ADJCALLSTACKDOWN : MOSInst<(outs), (ins i16imm:$amt, i16imm:$amt2), "#ADJCALLSTACKDOWN",
                                 [(MOScallseq_start timm:$amt, timm:$amt2)]>;
  def ADJCALLSTACKUP : MOSInst<(outs), (ins i16imm:$amt1, i16imm:$amt2), "#ADJCALLSTACKUP",
                               [(MOScallseq_end timm:$amt1, timm:$amt2)]>;
}

// rs0 += n, two's complement: the prologue, the epilogue and a call sequence that is not reserved.
let Defs = [P] in
  def ADJSP : MOSInst<(outs), (ins i16imm:$n), "adjsp $n", []>;
// d := rs0 + off: the address of a frame object once its frame index is resolved.
let Defs = [P] in
  def LEASP16 : MOSInst<(outs Imag16:$d), (ins i16imm:$off), "leasp $d, $off", []>;

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
  def RET : MOSInst<(outs), (ins), "rts", [(MOSretglue)]>;

def MOSInstrInfo : InstrInfo;
def MOS : Target { let InstructionSet = MOSInstrInfo; }

defm : RemapAllTargetPseudoPointerOperands<Imag16>;

// A word's bytes are the two halves of its pair.
def : Pat<(i8 (trunc Imag16:$a)), (EXTRACT_SUBREG Imag16:$a, sub_lo)>;
def : Pat<(i8 (trunc (srl Imag16:$a, (i8 8)))), (EXTRACT_SUBREG Imag16:$a, sub_hi)>;
// Only the low byte of a left shift is wanted: shift the low byte (`(x << 11) >> 8` is a byte shifted by 3).
def : Pat<(i8 (trunc (shl Imag16:$a, (i8 imm:$k)))), (SHL8 (EXTRACT_SUBREG Imag16:$a, sub_lo), imm:$k)>;
def : Pat<(i16 (zext Imag8:$a)), (REG_SEQUENCE Imag16, Imag8:$a, sub_lo, (LDI8 0), sub_hi)>;
def : Pat<(i16 (anyext Imag8:$a)), (REG_SEQUENCE Imag16, Imag8:$a, sub_lo, (LDI8 0), sub_hi)>;
// A byte loaded and widened: the load, and a zero high byte.
def : Pat<(i16 (zextloadi8 MOSAddr:$p)), (REG_SEQUENCE Imag16, (LDP8 MOSAddr:$p), sub_lo, (LDI8 0), sub_hi)>;
def : Pat<(i16 (extloadi8 MOSAddr:$p)), (REG_SEQUENCE Imag16, (LDP8 MOSAddr:$p), sub_lo, (LDI8 0), sub_hi)>;
let AddedComplexity = 10 in {
  def : Pat<(i16 (zextloadi8 MOSIdx:$a)), (REG_SEQUENCE Imag16, (LDPI8 MOSIdx:$a), sub_lo, (LDI8 0), sub_hi)>;
  def : Pat<(i16 (extloadi8 MOSIdx:$a)), (REG_SEQUENCE Imag16, (LDPI8 MOSIdx:$a), sub_lo, (LDI8 0), sub_hi)>;
}
// The builder joins two bytes as `lo | (hi << 8)` over either kind of extension: that is the pair.
def ext8 : PatFrags<(ops node:$a), [(zext node:$a), (anyext node:$a)]>;
def : Pat<(i16 (or (ext8 Imag8:$lo), (shl (ext8 Imag8:$hi), (i8 8)))),
          (REG_SEQUENCE Imag16, Imag8:$lo, sub_lo, Imag8:$hi, sub_hi)>;

// A global's address is a 16-bit immediate the linker fills in.
def : Pat<(i16 (MOSwrapper tglobaladdr:$a)), (LDI16 tglobaladdr:$a)>;

// The address of a frame object: rs0 plus its offset, once the frame is laid out.
def : Pat<(i16 frameindex:$fi), (LEASP16 (to_tframeindex $fi))>;

// A signed byte widens by its sign: the high byte is the byte shifted right by 7.
def : Pat<(i16 (sext Imag8:$a)), (REG_SEQUENCE Imag16, Imag8:$a, sub_lo, (SRA8 Imag8:$a, 7), sub_hi)>;

// 32-bit multiply, divide, remainder and shifts by a variable: the type legalizer would split them
// into halves it cannot recombine, so they are one node of two result words, a call of the runtime
// where emitted (runtime/mos_rt.s, `__sa` and `__sb`).
def SDTMOSLong : SDTypeProfile<2, 4, [SDTCisVT<0, i16>, SDTCisVT<1, i16>, SDTCisVT<2, i16>, SDTCisVT<3, i16>,
                                     SDTCisVT<4, i16>, SDTCisVT<5, i16>]>;
def SDTMOSLongShift : SDTypeProfile<2, 3, [SDTCisVT<0, i16>, SDTCisVT<1, i16>, SDTCisVT<2, i16>, SDTCisVT<3, i16>,
                                          SDTCisVT<4, i8>]>;
def MOSmul32 : SDNode<"MOSISD::MUL32", SDTMOSLong>;
def MOSudiv32 : SDNode<"MOSISD::UDIV32", SDTMOSLong>;
def MOSsdiv32 : SDNode<"MOSISD::SDIV32", SDTMOSLong>;
def MOSurem32 : SDNode<"MOSISD::UREM32", SDTMOSLong>;
def MOSsrem32 : SDNode<"MOSISD::SREM32", SDTMOSLong>;
def MOSshl32 : SDNode<"MOSISD::SHL32", SDTMOSLongShift>;
def MOSsrl32 : SDNode<"MOSISD::SRL32", SDTMOSLongShift>;
def MOSsra32 : SDNode<"MOSISD::SRA32", SDTMOSLongShift>;

// No `Defs = [P]`: an implicit result would be a third result type, which the generator rejects.
let isCodeGenOnly = 0 in {
  def MUL32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag16:$bl, Imag16:$bh), "mul32",
      [(set i16:$dl, i16:$dh, (MOSmul32 i16:$al, i16:$ah, i16:$bl, i16:$bh))]>;
  def UDIV32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag16:$bl, Imag16:$bh), "udiv32",
      [(set i16:$dl, i16:$dh, (MOSudiv32 i16:$al, i16:$ah, i16:$bl, i16:$bh))]>;
  def SDIV32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag16:$bl, Imag16:$bh), "sdiv32",
      [(set i16:$dl, i16:$dh, (MOSsdiv32 i16:$al, i16:$ah, i16:$bl, i16:$bh))]>;
  def UREM32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag16:$bl, Imag16:$bh), "urem32",
      [(set i16:$dl, i16:$dh, (MOSurem32 i16:$al, i16:$ah, i16:$bl, i16:$bh))]>;
  def SREM32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag16:$bl, Imag16:$bh), "srem32",
      [(set i16:$dl, i16:$dh, (MOSsrem32 i16:$al, i16:$ah, i16:$bl, i16:$bh))]>;
  def SHL32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag8:$b), "shl32",
      [(set i16:$dl, i16:$dh, (MOSshl32 i16:$al, i16:$ah, i8:$b))]>;
  def SRL32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag8:$b), "srl32",
      [(set i16:$dl, i16:$dh, (MOSsrl32 i16:$al, i16:$ah, i8:$b))]>;
  def SRA32 : MOSInst<(outs Imag16:$dl, Imag16:$dh), (ins Imag16:$al, Imag16:$ah, Imag8:$b), "sra32",
      [(set i16:$dl, i16:$dh, (MOSsra32 i16:$al, i16:$ah, i8:$b))]>;
}
