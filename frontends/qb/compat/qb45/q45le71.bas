' Explicit LET assigns opaque scalar values and copies a same-type record.
type PairType
    countValue as integer
    totalValue as long
    tagValue as string * 3
end type

dim inputCount as integer
dim inputTotal as long
dim inputText as string
dim countValue as integer
dim totalValue as long
dim textValue as string
dim sourcePair as PairType
dim copiedPair as PairType

read inputCount, inputTotal, inputText
let countValue = inputCount
let totalValue = inputTotal
let textValue = inputText
let sourcePair.countValue = inputCount + 2
let sourcePair.totalValue = inputTotal - 7
let sourcePair.tagValue = left$(inputText, 3)
let copiedPair = sourcePair

sourcePair.countValue = 0
sourcePair.totalValue = 0
sourcePair.tagValue = "BAD"
if countValue <> -1234 then
    print "FAIL let integer"
    end
end if
if totalValue <> 100007 then
    print "FAIL let long"
    end
end if
if textValue <> "QB45" then
    print "FAIL let string"
    end
end if
if copiedPair.countValue <> -1232 or copiedPair.totalValue <> 100000 then
    print "FAIL let record numbers"
    end
end if
if copiedPair.tagValue <> "QB4" then
    print "FAIL let record string"
    end
end if
print "PASS let"
end

data -1234, 100007, "QB45"
