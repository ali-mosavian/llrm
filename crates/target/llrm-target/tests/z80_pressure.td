include "llvm/Target/Target.td"
class Z80Reg<string n> : Register<n> { let Namespace = "Z80"; }
class Z80Reg16<string n, list<Register> subs> : RegisterWithSubRegs<n, subs> { let Namespace = "Z80"; }
def sub_lo : SubRegIndex<8>;
def sub_hi : SubRegIndex<8, 8>;
def A : Z80Reg<"a">; def B : Z80Reg<"b">; def C : Z80Reg<"c">;
def D : Z80Reg<"d">; def E : Z80Reg<"e">; def H : Z80Reg<"h">; def L : Z80Reg<"l">;
let SubRegIndices = [sub_hi, sub_lo] in {
def BC : Z80Reg16<"bc", [B, C]>;
def DE : Z80Reg16<"de", [D, E]>;
def HL : Z80Reg16<"hl", [H, L]>;
}
def IY : Z80Reg<"iy">;
def GR8 : RegisterClass<"Z80", [i8], 8, (add B, C, D, E, H, L, A)>;
def ACC : RegisterClass<"Z80", [i8], 8, (add A)>;
def GR16 : RegisterClass<"Z80", [i16], 16, (add BC, DE, HL)>;
def HLR : RegisterClass<"Z80", [i16], 16, (add HL)>;
def IYR : RegisterClass<"Z80", [i16], 16, (add IY)>;
def Z80 : Target;
