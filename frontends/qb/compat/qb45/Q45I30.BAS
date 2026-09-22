' Pure numeric, string, and binary conversion intrinsics.
dim packedInteger as string * 2
dim packedLong as string * 4
dim text as string
dim passed as integer

packedInteger = mki$(3210)
packedLong = mkl$(100000)
text = chr$(asc("Q")) + lcase$("B") + ucase$("a")
passed = abs(-7) = 7 and sgn(-5) = -1 and fix(-1.9) = -1 and int(-1.9) = -2
passed = passed and sqr(81) = 9 and cvi(packedInteger) = 3210 and cvl(packedLong) = 100000
passed = passed and asc(mid$(packedInteger, 1, 1)) = 138
passed = passed and asc(mid$(packedInteger, 2, 1)) = 12
passed = passed and asc(mid$(packedLong, 1, 1)) = 160
passed = passed and asc(mid$(packedLong, 2, 1)) = 134
passed = passed and asc(mid$(packedLong, 3, 1)) = 1
passed = passed and asc(mid$(packedLong, 4, 1)) = 0
passed = passed and text = "QbA" and instr("QuickBASIC", "BASIC") = 6
passed = passed and left$("abcd", 2) = "ab" and right$("abcd", 2) = "cd"
passed = passed and mid$("abcd", 2, 2) = "bc" and hex$(255) = "FF" and oct$(8) = "10"
passed = passed and val(" 42x") = 42 and len(space$(3)) = 3 and string$(3, "x") = "xxx"

if passed then
    print "PASS intrinsics"
else
    print "FAIL intrinsics values"
end if
end
