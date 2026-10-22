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
