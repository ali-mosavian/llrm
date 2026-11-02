; zero-page cells and block layout shared by sf32.s and sf32_muldiv.s
rc2 = $82
rc3 = $83
rc4 = $84
rc5 = $85
rc6 = $86
rc7 = $87
rc8 = $88
rc9 = $89
rc16 = $90
rc17 = $91
rc18 = $92
rc19 = $93
rc20 = $94
rc21 = $95
rc22 = $96
rc23 = $97
rc24 = $98
rc25 = $99

; operand blocks: guard, m0, m1, m2 (m2 bit 7 is the implicit one when normal)
blkA = rc16
blkB = rc20
ea   = rc24
eb   = rc25
sa   = rc2              ; sign of a, $80 or 0 (valid once a is unpacked)
sb   = rc3
re   = rc4              ; result exponent
rsg  = rc5              ; result sign
tmp  = rc6
tm2  = rc7
fl   = rc16             ; special path only: blocks are dead there
ca   = rc17
fx   = rc18
