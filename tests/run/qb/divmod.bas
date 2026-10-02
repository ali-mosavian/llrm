' Divide and remainder are calls into the runtime, and neither is commutative,
' so an operand order that is backwards is a different answer. BASIC truncates
' toward zero and the remainder takes the dividend's sign, which is idiv's
' behaviour -- so this also guards any later strength reduction to sar, which
' gets -7 \ 2 wrong by one.
'
' The last two are the reason divide may not simply become idiv: the runtime
' raises a BASIC error where the instruction raises #DE, and they need not agree.
DEFINT A-Z
DIM a AS LONG, b AS LONG, r AS LONG, zero AS LONG, low AS LONG
DIM caught AS INTEGER

ON ERROR GOTO handler

a = 7
b = 2
PRINT "DIV1="; a \ b
PRINT "MOD1="; a MOD b
a = -7
PRINT "DIV2="; a \ b
PRINT "MOD2="; a MOD b
b = -2
PRINT "DIV3="; a \ b
PRINT "MOD3="; a MOD b
a = 7
PRINT "DIV4="; a \ b
PRINT "MOD4="; a MOD b

a = 305419896
b = 252645135
PRINT "DIVBIG="; b \ a
PRINT "MODBIG="; b MOD a
PRINT "MULSMALL="; a * 3

' A literal power-of-two divisor is the only shape the shift-with-bias
' strength reduction fires on -- every divisor above is a variable, so none
' of them reaches it. A sar alone rounds towards minus infinity and gets
' each negative case here wrong by one.
r = -7
PRINT "SHDIV1="; r \ 2
PRINT "SHMOD1="; r MOD 2
PRINT "SHDIV2="; r \ 512
PRINT "SHMOD2="; r MOD 512
r = -1000000
PRINT "SHDIV3="; r \ 512
PRINT "SHMOD3="; r MOD 512
r = 1000000
PRINT "SHDIV4="; r \ 512
PRINT "SHMOD4="; r MOD 512

a = 305419896
b = 252645135
caught = 0
r = a * b
PRINT "MULOVF="; caught

PRINT "DONE"
END

handler:
caught = ERR
RESUME NEXT
