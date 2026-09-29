if getB%(80, 0) + getB%(80, 1) = 0 then
    print #1, "SKIP CURRENCY: pCur never called"
else
    checkHex 80, "pCur h", "1111"
    checkHex 81, "pCur v", "FFFFFFFFFF439EB2"
    checkHex 82, "pCur t", "2222"
    checkHex 83, "pCur back", "FFFFFFFFFE873D64"
    checkHex 84, "rCur h", "3333"
    checkHex 85, "rCur", "0000000005E30A78"
end if
