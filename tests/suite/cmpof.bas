' Signed LONG comparison where BC's own runtime gets it wrong, and qbopt
' does not. This program is in configs.DIVERGES: BC's build disagrees with
' the golden on purpose, and only the rewritten build is expected to match.
'
' B$CPI4 (runtime/rt/helpi4.asm) compares the high words signed, and where
' they are equal falls through to compare the low words UNSIGNED. It then
' rebuilds a signed answer out of the unsigned flags:
'
'       cmp ax,word ptr (op2)   ; sets OF from a SIGNED 16-bit compare
'       lahf                    ; ah = SF ZF _ AF _ PF _ CF
'       and ax,4100h            ; keep ZF and CF
'       shr ax,1                ; move CF into SF's position
'       or  ah,al
'       sahf                    ; loads SF ZF AF PF CF -- the LOW byte only
'
' sahf cannot write OF, which lives in bit 11. So OF survives from that
' low-word compare, and every jl/jle/jg/jge afterwards -- all of which read
' SF <> OF -- answers against a flag left over from an unrelated comparison.
' ZF is never touched, which is exactly why = and <> stay correct here while
' <, <=, > and >= do not.
'
' It bites when the high words are equal and the low words straddle 0x8000
' far enough for the 16-bit signed subtraction to overflow. Every pair below
' is two CONSECUTIVE integers, so this is not an exotic-value bug: it is any
' two longs that happen to sit either side of a low-half boundary. Its
' absence from cmpord.bas is a near miss -- that program's own "identical
' high halves" pair is &H12340000 against &H1234FFFF, whose low words do not
' overflow, so it passes.
DEFINT A-Z
DIM e1 AS LONG, e2 AS LONG
DIM f1 AS LONG, f2 AS LONG
DIM g1 AS LONG, g2 AS LONG
e1 = 305430527   ' &H12347FFF
e2 = 305430528   ' &H12348000
f1 = 2147450879  ' &H7FFF7FFF, up against the positive extreme
f2 = 2147450880  ' &H7FFF8000
g1 = -268402689  ' &HF0007FFF, a negative high word
g2 = -268402688  ' &HF0008000

PRINT "ELT="; (e1 < e2); (e2 < e1)
PRINT "ELE="; (e1 <= e2); (e2 <= e1)
PRINT "EGT="; (e1 > e2); (e2 > e1)
PRINT "EGE="; (e1 >= e2); (e2 >= e1)
PRINT "EEQ="; (e1 = e2); (e2 = e1)
PRINT "ENE="; (e1 <> e2); (e2 <> e1)
PRINT "FLT="; (f1 < f2); (f2 < f1)
PRINT "FLE="; (f1 <= f2); (f2 <= f1)
PRINT "FGT="; (f1 > f2); (f2 > f1)
PRINT "FGE="; (f1 >= f2); (f2 >= f1)
PRINT "FEQ="; (f1 = f2); (f2 = f1)
PRINT "FNE="; (f1 <> f2); (f2 <> f1)
PRINT "GLT="; (g1 < g2); (g2 < g1)
PRINT "GLE="; (g1 <= g2); (g2 <= g1)
PRINT "GGT="; (g1 > g2); (g2 > g1)
PRINT "GGE="; (g1 >= g2); (g2 >= g1)
PRINT "GEQ="; (g1 = g2); (g2 = g1)
PRINT "GNE="; (g1 <> g2); (g2 <> g1)
PRINT "DONE"
