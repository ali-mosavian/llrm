// Toy 6502 target for the DAG-selector spike: i8 in zero-page "imaginary"
// registers (llvm-mos convention), i16 pointers as register pairs, carry as
// the flags register P. Shaped after LLVM's AVR target.

include "llvm/Target/Target.td"

def sub_lo : SubRegIndex<8>;
def sub_hi : SubRegIndex<8, 8>;

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
def MOSAddr : ComplexPattern<i16, 1, "SelectIndirect", [], []>;

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
def MOSbrcond : SDNode<"MOSISD::BRCOND", SDTMOSBrcond, [SDNPHasChain]>;
def MOSsetcc : SDNode<"MOSISD::SETCC", SDTMOSSetcc>;
def MOScmov : SDNode<"MOSISD::CMOV", SDTMOSCmov>;

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

def LDI8 : MOSInst<(outs Imag8:$d), (ins i8imm:$v), "ldi $d, $v", [(set i8:$d, imm:$v)]>;
def LDI16 : MOSInst<(outs Imag16:$d), (ins i16imm:$v), "ldi $d, $v", [(set i16:$d, imm:$v)]>;

// LDY #0; LDA (p),Y; STA d.
let mayLoad = 1 in
  def LDP8 : MOSInst<(outs Imag8:$d), (ins Imag16:$p), "ldp $d, ($p)", [(set i8:$d, (load MOSAddr:$p))]>;
let mayStore = 1 in
  def STP8 : MOSInst<(outs), (ins Imag8:$s, Imag16:$p), "stp $s, ($p)", [(store i8:$s, MOSAddr:$p)]>;
let mayStore = 1, Predicates = [HasCMOS] in
  def STZP : MOSInst<(outs), (ins Imag16:$p), "stz ($p)", [(store (i8 0), MOSAddr:$p)]>;

def brtarget : Operand<OtherVT>;
def calltarget : Operand<i16>;

// Instructions the selector does not match from a pattern: copies, compare, branches, call. They exist
// so the emitter is keyed to generated opcode ids.
def COPY8 : MOSInst<(outs Reg8:$d), (ins Reg8:$s), "copy $d, $s", []>;
def COPY16 : MOSInst<(outs Imag16:$d), (ins Imag16:$s), "copy $d, $s", []>;
// LDA a; CMP b: the flags of a - b.
def CMP8 : MOSInst<(outs Flags:$f), (ins Imag8:$a, Imag8:$b), "cmp $f, $a, $b", [(set i8:$f, (MOScmp i8:$a, i8:$b))]>;
let isBranch = 1, isTerminator = 1 in {
  def BEQ : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "beq $t", [(MOSbrcond bb:$t, (i8 0), i8:$f)]>;
  def BNE : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bne $t", [(MOSbrcond bb:$t, (i8 1), i8:$f)]>;
  def BCC : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bcc $t", [(MOSbrcond bb:$t, (i8 2), i8:$f)]>;
  def BCS : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bcs $t", [(MOSbrcond bb:$t, (i8 3), i8:$f)]>;
  def BMI : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bmi $t", [(MOSbrcond bb:$t, (i8 4), i8:$f)]>;
  def BPL : MOSInst<(outs), (ins brtarget:$t, Flags:$f), "bpl $t", [(MOSbrcond bb:$t, (i8 5), i8:$f)]>;
}
// A condition as a byte 0 or 1, and a select: branches in the emitted code, flags saved around the loads.
def SETCC8 : MOSInst<(outs Imag8:$d), (ins i8imm:$cc, Flags:$f), "setcc $d, $cc, $f", [(set i8:$d, (MOSsetcc imm:$cc, i8:$f))]>;
def CMOV8 : MOSInst<(outs Imag8:$d), (ins Imag8:$f0, Imag8:$t, i8imm:$cc, Flags:$f), "cmov $d, $f0, $t, $cc, $f",
                    [(set i8:$d, (MOScmov i8:$f0, i8:$t, imm:$cc, i8:$f))]>;
let isBranch = 1, isTerminator = 1, isBarrier = 1 in
  def JMP : MOSInst<(outs), (ins brtarget:$t), "jmp $t", []>;
let isCall = 1 in
  def JSR : MOSInst<(outs), (ins calltarget:$t), "jsr $t", []>;

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
  def RET : MOSInst<(outs), (ins), "rts", [(MOSretglue)]>;

def MOSInstrInfo : InstrInfo;
def MOS : Target { let InstructionSet = MOSInstrInfo; }

defm : RemapAllTargetPseudoPointerOperands<Imag16>;
