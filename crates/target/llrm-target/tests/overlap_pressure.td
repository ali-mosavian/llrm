include "llvm/Target/Target.td"
class R<string n> : Register<n> { let Namespace = "Ov"; }
foreach i = 0-11 in def R#i : R<"r"#i>;
def P : RegisterClass<"Ov", [i32], 32, (sequence "R%u", 0, 5)>;
def Q : RegisterClass<"Ov", [i32], 32, (sequence "R%u", 4, 11)>;
def S : RegisterClass<"Ov", [i32], 32, (add R0, R1)>;
def Ov : Target;
