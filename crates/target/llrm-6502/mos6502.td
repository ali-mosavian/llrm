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

foreach i = 0-7 in def RC#i : MOSReg<"rc"#i>;
foreach i = 0-3 in def RS#i : MOSPair<"rs"#i, [!cast<Register>("RC"#!mul(i, 2)), !cast<Register>("RC"#!add(!mul(i, 2), 1))]>;
def P : MOSReg<"p">;

def Imag8 : RegisterClass<"MOS", [i8], 8, (sequence "RC%u", 0, 7)>;
def Imag16 : RegisterClass<"MOS", [i16], 8, (sequence "RS%u", 0, 3)>;
def Flags : RegisterClass<"MOS", [i8], 8, (add P)> { let CopyCost = -1; }

// 65C02 adds STZ, among others.
def HasCMOS : Predicate<"Subtarget->hasCMOS()">;

// (zp),Y addressing: a pointer in an Imag16 pair.
def MOSAddr : ComplexPattern<i16, 1, "SelectIndirect", [], []>;

def MOSretglue : SDNode<"MOSISD::RET_GLUE", SDTNone, [SDNPHasChain, SDNPOptInGlue, SDNPVariadic]>;

class MOSInst<dag outs, dag ins, string asm, list<dag> pattern> : Instruction {
  let Namespace = "MOS";
  let OutOperandList = outs;
  let InOperandList = ins;
  let AsmString = asm;
  let Pattern = pattern;
}

// CLC; LDA a; ADC b; STA d.
let Defs = [P] in {
  def ADD8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a, Imag8:$b), "add $d, $a, $b",
                     [(set i8:$d, (add i8:$a, i8:$b))]>;
  def ADDC8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a, Imag8:$b), "addc $d, $a, $b",
                      [(set i8:$d, (addc i8:$a, i8:$b))]>;
  def SUB8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a, Imag8:$b), "sub $d, $a, $b",
                     [(set i8:$d, (sub i8:$a, i8:$b))]>;
}
// LDA a; ADC b; STA d, carry in from P.
let Uses = [P], Defs = [P] in
  def ADDE8 : MOSInst<(outs Imag8:$d), (ins Imag8:$a, Imag8:$b), "adde $d, $a, $b",
                      [(set i8:$d, (adde i8:$a, i8:$b))]>;

def LDI8 : MOSInst<(outs Imag8:$d), (ins i8imm:$v), "ldi $d, $v", [(set i8:$d, imm:$v)]>;

// LDY #0; LDA (p),Y; STA d.
let mayLoad = 1 in
  def LDP8 : MOSInst<(outs Imag8:$d), (ins Imag16:$p), "ldp $d, ($p)", [(set i8:$d, (load MOSAddr:$p))]>;
let mayStore = 1 in
  def STP8 : MOSInst<(outs), (ins Imag8:$s, Imag16:$p), "stp $s, ($p)", [(store i8:$s, MOSAddr:$p)]>;
let mayStore = 1, Predicates = [HasCMOS] in
  def STZP : MOSInst<(outs), (ins Imag16:$p), "stz ($p)", [(store (i8 0), MOSAddr:$p)]>;

let isReturn = 1, isTerminator = 1, isBarrier = 1 in
  def RET : MOSInst<(outs), (ins), "rts", [(MOSretglue)]>;

def MOSInstrInfo : InstrInfo;
def MOS : Target { let InstructionSet = MOSInstrInfo; }

defm : RemapAllTargetPseudoPointerOperands<Imag16>;
