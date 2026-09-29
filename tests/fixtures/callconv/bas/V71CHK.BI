if getB%(86, 0) + getB%(86, 1) = 0 then
    print #1, "SKIP BYVAL definitions: pByv never called"
else
    checkHex 86, "pByv h", "1111"
    checkHex 87, "pByv v", "EDCBA987"
    checkHex 88, "pByv s", "3FC00000"
    checkHex 89, "pByv d", "C002000000000000"
    checkHex 93, "pByv t", "2222"
    checkHex 97, "pByv h after", "1111"
    checkHex 98, "pByv v after", "EDCBA987"
end if
