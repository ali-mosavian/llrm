// Z80 for the DAG selector. LLVM has no Z80 target, so this is written here, shaped after the AVR target and
// the 6502 `.td`: 8-bit A B C D E H L, the pairs BC DE HL (low byte second: HL = H:L), IX/IY, SP and the flags.
// The 8-bit ALU works on A only and 16-bit add on HL only, so those instructions take single-register classes
// (as x86's AL ops do) and the allocator places the copies.

include "llvm/Target/Target.td"

def sub_lo : SubRegIndex<8> { let Namespace = "Z80"; }
def sub_hi : SubRegIndex<8, 8> { let Namespace = "Z80"; }

class Z80Reg<string n> : Register<n> { let Namespace = "Z80"; }
class Z80Pair<string n, Register hi, Register lo> : RegisterWithSubRegs<n, [lo, hi]> {
  let Namespace = "Z80";
  let SubRegIndices = [sub_lo, sub_hi];
}

def A : Z80Reg<"a">;
def B : Z80Reg<"b">;
def C : Z80Reg<"c">;
def D : Z80Reg<"d">;
def E : Z80Reg<"e">;
def H : Z80Reg<"h">;
def L : Z80Reg<"l">;
def BC : Z80Pair<"bc", B, C>;
def DE : Z80Pair<"de", D, E>;
def HL : Z80Pair<"hl", H, L>;
def IX : Z80Reg<"ix">;
def IY : Z80Reg<"iy">;
def SP : Z80Reg<"sp">;
def F : Z80Reg<"f">;

def GR8 : RegisterClass<"Z80", [i8], 8, (add A, B, C, D, E, H, L)>;
def ACC : RegisterClass<"Z80", [i8], 8, (add A)>;
def GR16 : RegisterClass<"Z80", [i16], 8, (add BC, DE, HL)>;
def HLR : RegisterClass<"Z80", [i16], 8, (add HL)>;
def IDX : RegisterClass<"Z80", [i16], 8, (add IX, IY)>;
def SPR : RegisterClass<"Z80", [i16], 8, (add SP)>;
// The flags as an i8 value, as the 6502's P and m68k's CCR are. The condition code is a constant:
// NZ 0, Z 1, NC 2 (unsigned >=), C 3 (unsigned <).
def Flags : RegisterClass<"Z80", [i8], 8, (add F)> { let CopyCost = -1; }

def SDTZ80Cmp : SDTypeProfile<1, 2, [SDTCisVT<0, i8>, SDTCisSameAs<1, 2>]>;
def SDTZ80Brcond : SDTypeProfile<0, 3, [SDTCisVT<0, OtherVT>, SDTCisVT<1, i8>, SDTCisVT<2, i8>]>;
def SDTZ80Setcc : SDTypeProfile<1, 2, [SDTCisVT<0, i8>, SDTCisVT<1, i8>, SDTCisVT<2, i8>]>;
def SDTZ80Cmov : SDTypeProfile<1, 4, [SDTCisSameAs<0, 1>, SDTCisSameAs<1, 2>, SDTCisVT<3, i8>, SDTCisVT<4, i8>]>;
def Z80setcc : SDNode<"Z80ISD::SETCC", SDTZ80Setcc>;
def Z80cmov : SDNode<"Z80ISD::CMOV", SDTZ80Cmov>;
def Z80cmp : SDNode<"Z80ISD::CMP", SDTZ80Cmp>;
def Z80brcond : SDNode<"Z80ISD::BRCOND", SDTZ80Brcond, [SDNPHasChain]>;
def SDT_Z80CallSeqStart : SDCallSeqStart<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def SDT_Z80CallSeqEnd : SDCallSeqEnd<[SDTCisVT<0, i16>, SDTCisVT<1, i16>]>;
def Z80callseq_start : SDNode<"ISD::CALLSEQ_START", SDT_Z80CallSeqStart, [SDNPHasChain, SDNPOutGlue]>;
def Z80callseq_end : SDNode<"ISD::CALLSEQ_END", SDT_Z80CallSeqEnd, [SDNPHasChain, SDNPOptInGlue, SDNPOutGlue]>;
def SDT_Z80Call : SDTypeProfile<0, -1, [SDTCisVT<0, i16>]>;
def Z80call : SDNode<"Z80ISD::CALL", SDT_Z80Call, [SDNPHasChain, SDNPOutGlue, SDNPOptInGlue, SDNPVariadic]>;
def Z80wrapper : SDNode<"Z80ISD::WRAPPER", SDTIntUnaryOp>;
def Z80retglue : SDNode<"Z80ISD::RET_GLUE", SDTNone, [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;

class Z80Inst<dag outs, dag ins, string asm, list<dag> pattern> : Instruction {
  let Namespace = "Z80";
  let OutOperandList = outs;
  let InOperandList = ins;
  let AsmString = asm;
  let Pattern = pattern;
}

// The Z180 adds mlt rr (B*C into BC), the first multiply.
def HasZ180 : Predicate<"Subtarget->hasZ180()">;

def brtarget : Operand<OtherVT>;
def calltarget : Operand<i16>;

// A op r and A op n: ADD, SUB, AND, OR, XOR. Every one writes the flags and leaves the result in A.
multiclass ALU8<string n, SDPatternOperator op> {
  let Defs = [F], Constraints = "$d = $a" in {
    def NAME#"8r" : Z80Inst<(outs ACC:$d), (ins ACC:$a, GR8:$b), n#" a, $b", [(set i8:$d, (op i8:$a, i8:$b))]>;
    def NAME#"8i" : Z80Inst<(outs ACC:$d), (ins ACC:$a, i8imm:$b), n#" a, $b", [(set i8:$d, (op i8:$a, (i8 imm:$b)))]>;
  }
}
defm ADD : ALU8<"add", add>;
defm SUB : ALU8<"sub", sub>;
defm AND : ALU8<"and", and>;
defm OR : ALU8<"or", or>;
defm XOR : ALU8<"xor", xor>;

// inc r / dec r: any 8-bit register, flags written except carry.
let Defs = [F], Constraints = "$d = $a" in {
  def INC8 : Z80Inst<(outs GR8:$d), (ins GR8:$a), "inc $d", [(set i8:$d, (add i8:$a, (i8 1)))]>;
  def DEC8 : Z80Inst<(outs GR8:$d), (ins GR8:$a), "dec $d", [(set i8:$d, (add i8:$a, (i8 -1)))]>;
}

// add hl,rr: the one 16-bit add. inc rr / dec rr leave the flags alone.
let Defs = [F], Constraints = "$d = $a" in
  def ADD16 : Z80Inst<(outs HLR:$d), (ins HLR:$a, GR16:$b), "add hl, $b", [(set i16:$d, (add i16:$a, i16:$b))]>;
let Constraints = "$d = $a" in {
  def INC16 : Z80Inst<(outs GR16:$d), (ins GR16:$a), "inc $d", [(set i16:$d, (add i16:$a, (i16 1)))]>;
  def DEC16 : Z80Inst<(outs GR16:$d), (ins GR16:$a), "dec $d", [(set i16:$d, (add i16:$a, (i16 -1)))]>;
}

// The full product of two zero-extended bytes; emission builds B:C and runs mlt bc.
let Predicates = [HasZ180], Defs = [F] in
  def UMUL8X8 : Z80Inst<(outs GR16:$d), (ins GR8:$a, GR8:$b), "umul8x8 $d, $a, $b",
                        [(set i16:$d, (mul (zext i8:$a), (zext i8:$b)))]>;


// Word logic is the two bytes' ops, expanded when emitted (the 6502's OR16 is the same); the join of two bytes
// into a pair, `lo | (hi << 8)`, is the more specific pattern below and wins.
multiclass Logic16<string n, SDPatternOperator op> {
  let Defs = [F] in
    def NAME#"16" : Z80Inst<(outs GR16:$d), (ins GR16:$a, GR16:$b), n#"16 $d, $a, $b", [(set i16:$d, (op i16:$a, i16:$b))]>;
}
defm AND : Logic16<"and", and>;
defm OR : Logic16<"or", or>;
defm XOR : Logic16<"xor", xor>;

// sub hl,rr: or a; sbc hl,rr. The carry out of an add is not an operand here, so a word add and subtract are the
// one instruction each; i32 and i64 come in through the carry forms later.
let Defs = [F], Constraints = "$d = $a" in
  def SUB16 : Z80Inst<(outs HLR:$d), (ins HLR:$a, GR16:$b), "sub hl, $b", [(set i16:$d, (sub i16:$a, i16:$b))]>;

// Shifts by a constant: one SLA/SRL/SRA (or add hl,hl, srl h; rr l) per bit, expanded when emitted.
multiclass Shift<string n, RegisterClass rc, ValueType vt> {
  let Defs = [F], Constraints = "$d = $a" in {
    def SHL#n : Z80Inst<(outs rc:$d), (ins rc:$a, i8imm:$k), "shl $d, $k", [(set vt:$d, (shl vt:$a, (i8 imm:$k)))]>;
    def SRL#n : Z80Inst<(outs rc:$d), (ins rc:$a, i8imm:$k), "srl $d, $k", [(set vt:$d, (srl vt:$a, (i8 imm:$k)))]>;
    def SRA#n : Z80Inst<(outs rc:$d), (ins rc:$a, i8imm:$k), "sra $d, $k", [(set vt:$d, (sra vt:$a, (i8 imm:$k)))]>;
  }
}
defm "" : Shift<"8", GR8, i8>;
defm "" : Shift<"16", GR16, i16>;

def LD8ri : Z80Inst<(outs GR8:$d), (ins i8imm:$v), "ld $d, $v", [(set i8:$d, imm:$v)]>;
def LD16ri : Z80Inst<(outs GR16:$d), (ins i16imm:$v), "ld $d, $v", [(set i16:$d, imm:$v)]>;

// ld r,(hl) / ld (hl),r / ld (hl),n: the pointer is HL.
let mayLoad = 1 in
  def LD8rm : Z80Inst<(outs GR8:$d), (ins HLR:$p), "ld $d, ($p)", [(set i8:$d, (load i16:$p))]>;
// A word through a pointer in HL: two byte loads or stores with an `inc hl` between, expanded when emitted.
let mayLoad = 1 in
  def LD16rm : Z80Inst<(outs GR16:$d), (ins HLR:$p), "ld16 $d, ($p)", [(set i16:$d, (load i16:$p))]>;
let mayStore = 1 in
  def LD16mr : Z80Inst<(outs), (ins GR16:$s, HLR:$p), "ld16 ($p), $s", [(store i16:$s, i16:$p)]>;
let mayStore = 1 in {
  def LD8mr : Z80Inst<(outs), (ins GR8:$s, HLR:$p), "ld ($p), $s", [(store i8:$s, i16:$p)]>;
  def LD8mi : Z80Inst<(outs), (ins i8imm:$v, HLR:$p), "ld ($p), $v", [(store (i8 imm:$v), i16:$p)]>;
}
// ld a,(nn) / ld (nn),a / ld hl,(nn) / ld (nn),hl: a global.
let mayLoad = 1, AddedComplexity = 10 in {
  def LD8am : Z80Inst<(outs ACC:$d), (ins i16imm:$a), "ld a, ($a)", [(set i8:$d, (load (Z80wrapper tglobaladdr:$a)))]>;
  def LD16hm : Z80Inst<(outs HLR:$d), (ins i16imm:$a), "ld hl, ($a)", [(set i16:$d, (load (Z80wrapper tglobaladdr:$a)))]>;
}
let mayStore = 1, AddedComplexity = 10 in {
  def LD8ma : Z80Inst<(outs), (ins ACC:$s, i16imm:$a), "ld ($a), a", [(store i8:$s, (Z80wrapper tglobaladdr:$a))]>;
  def LD16mh : Z80Inst<(outs), (ins HLR:$s, i16imm:$a), "ld ($a), hl", [(store i16:$s, (Z80wrapper tglobaladdr:$a))]>;
}

// Copies the selector does not match from a pattern.
def COPY8 : Z80Inst<(outs GR8:$d), (ins GR8:$s), "copy $d, $s", []>;
def COPY16 : Z80Inst<(outs GR16:$d), (ins GR16:$s), "copy $d, $s", []>;

// cp r / cp n: the flags of a - b.
def CP8r : Z80Inst<(outs Flags:$f), (ins ACC:$a, GR8:$b), "cp $b", [(set i8:$f, (Z80cmp i8:$a, i8:$b))]>;
def CP8i : Z80Inst<(outs Flags:$f), (ins ACC:$a, i8imm:$b), "cp $b", [(set i8:$f, (Z80cmp i8:$a, (i8 imm:$b)))]>;
// The flags of a - b over a word: push hl; or a; sbc hl,rr; pop hl. HL is kept; Z covers both bytes (SBC HL sets it
// on the 16-bit result) and C is the unsigned borrow.
def CP16 : Z80Inst<(outs Flags:$f), (ins HLR:$a, GR16:$b), "cp16 $a, $b", [(set i8:$f, (Z80cmp i16:$a, i16:$b))]>;
let isBranch = 1, isTerminator = 1 in {
  def JPNZ : Z80Inst<(outs), (ins brtarget:$t, Flags:$f), "jp nz, $t", [(Z80brcond bb:$t, (i8 0), i8:$f)]>;
  def JPZ : Z80Inst<(outs), (ins brtarget:$t, Flags:$f), "jp z, $t", [(Z80brcond bb:$t, (i8 1), i8:$f)]>;
  def JPNC : Z80Inst<(outs), (ins brtarget:$t, Flags:$f), "jp nc, $t", [(Z80brcond bb:$t, (i8 2), i8:$f)]>;
  def JPC : Z80Inst<(outs), (ins brtarget:$t, Flags:$f), "jp c, $t", [(Z80brcond bb:$t, (i8 3), i8:$f)]>;
}
// A condition as a byte 0 or 1, and a select: branches in the emitted code; `ld` leaves the flags alone, so the
// loads between the compare and the branch are safe.
def SETCC8 : Z80Inst<(outs GR8:$d), (ins i8imm:$cc, Flags:$f), "setcc $d, $cc, $f", [(set i8:$d, (Z80setcc imm:$cc, i8:$f))]>;
def CMOV8 : Z80Inst<(outs GR8:$d), (ins GR8:$f0, GR8:$t, i8imm:$cc, Flags:$f), "cmov $d, $f0, $t, $cc, $f",
                    [(set i8:$d, (Z80cmov i8:$f0, i8:$t, imm:$cc, i8:$f))]>;
def CMOV16 : Z80Inst<(outs GR16:$d), (ins GR16:$f0, GR16:$t, i8imm:$cc, Flags:$f), "cmov16 $d, $f0, $t, $cc, $f",
                     [(set i16:$d, (Z80cmov i16:$f0, i16:$t, imm:$cc, i8:$f))]>;
// jp or jr: branch relaxation picks the short form once the distance is known.
let isBranch = 1, isTerminator = 1, isBarrier = 1 in
  def JP : Z80Inst<(outs), (ins brtarget:$t), "jp $t", [(br bb:$t)]>;

let isCall = 1 in
  def CALL : Z80Inst<(outs), (ins calltarget:$t), "call $t", [(Z80call tglobaladdr:$t)]>;
// a routine the selector names itself (the libcall of a fill or copy of unknown size)
def : Pat<(Z80call texternalsym:$t), (CALL texternalsym:$t)>;

// The call sequence's stack adjustment, resolved by the frame code once the frame is laid out.
let Defs = [SP, F], Uses = [SP] in {
  def ADJCALLSTACKDOWN : Z80Inst<(outs), (ins i16imm:$amt, i16imm:$amt2), "#ADJCALLSTACKDOWN",
                                 [(Z80callseq_start timm:$amt, timm:$amt2)]>;
  def ADJCALLSTACKUP : Z80Inst<(outs), (ins i16imm:$amt1, i16imm:$amt2), "#ADJCALLSTACKUP",
                               [(Z80callseq_end timm:$amt1, timm:$amt2)]>;
}

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
  def RET : Z80Inst<(outs), (ins), "ret", [(Z80retglue)]>;

def Z80InstrInfo : InstrInfo;
def Z80 : Target { let InstructionSet = Z80InstrInfo; }

defm : RemapAllTargetPseudoPointerOperands<GR16>;

// A word's bytes are the two halves of its pair.
def : Pat<(i8 (trunc GR16:$a)), (EXTRACT_SUBREG GR16:$a, sub_lo)>;
def : Pat<(i8 (trunc (srl GR16:$a, (i8 8)))), (EXTRACT_SUBREG GR16:$a, sub_hi)>;
def : Pat<(i16 (zext GR8:$a)), (REG_SEQUENCE GR16, GR8:$a, sub_lo, (LD8ri 0), sub_hi)>;
def : Pat<(i16 (anyext GR8:$a)), (REG_SEQUENCE GR16, GR8:$a, sub_lo, (LD8ri 0), sub_hi)>;

// A signed byte widens by its sign: the high byte is the byte shifted right by 7.
def : Pat<(i16 (sext GR8:$a)), (REG_SEQUENCE GR16, GR8:$a, sub_lo, (SRA8 GR8:$a, 7), sub_hi)>;

// A global's address is a 16-bit immediate the linker fills in.
def : Pat<(i16 (Z80wrapper tglobaladdr:$a)), (LD16ri tglobaladdr:$a)>;

// The builder joins two bytes as `lo | (hi << 8)` over either kind of extension: that is the pair.
def ext8 : PatFrags<(ops node:$a), [(zext node:$a), (anyext node:$a)]>;
def : Pat<(i16 (or (ext8 GR8:$lo), (shl (ext8 GR8:$hi), (i8 8)))),
          (REG_SEQUENCE GR16, GR8:$lo, sub_lo, GR8:$hi, sub_hi)>;

// A byte loaded and widened: the load, and a zero high byte.
def : Pat<(i16 (zextloadi8 i16:$p)), (REG_SEQUENCE GR16, (LD8rm i16:$p), sub_lo, (LD8ri 0), sub_hi)>;
def : Pat<(i16 (extloadi8 i16:$p)), (REG_SEQUENCE GR16, (LD8rm i16:$p), sub_lo, (LD8ri 0), sub_hi)>;
