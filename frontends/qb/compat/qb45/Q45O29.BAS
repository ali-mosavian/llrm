' QuickHelp operator truth-table and precedence probe.
dim passed as integer
dim twoValue as integer
dim threeValue as integer
dim fourValue as integer
dim sevenValue as integer
dim zeroValue as integer
dim leftText as string
dim rightText as string

read twoValue, threeValue, fourValue, sevenValue, zeroValue, leftText, rightText
passed = twoValue + threeValue * fourValue = 14
if passed then passed = 17 mod 5 = 2 and twoValue ^ threeValue = 8
if passed then passed = sevenValue > threeValue and sevenValue >= sevenValue
if passed then passed = threeValue < sevenValue and threeValue <= threeValue
if passed then passed = threeValue <> fourValue
if passed then passed = (not zeroValue) = -1
if passed then passed = (1 and threeValue) = 1 and (1 or twoValue) = 3
if passed then passed = (1 xor threeValue) = 2 and (1 eqv 1) = -1
if passed then passed = (zeroValue imp zeroValue) = -1
if passed then passed = leftText + rightText = "QB45"

if passed then
    print "PASS operators"
else
    print "FAIL operators truth"
end if
end

data 2, 3, 4, 7, 0, "QB", "45"
