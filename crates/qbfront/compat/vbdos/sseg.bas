option explicit

dim textValue as string
dim nullValue as string
dim segmentValue as integer
dim offsetValue as integer
dim farAddress as long
dim expectedAddress as long

textValue = "ABCD"
segmentValue = sseg(textValue)
offsetValue = sadd(textValue)
farAddress = ssegadd(textValue)
expectedAddress = clng(segmentValue) * 65536 + (clng(offsetValue) and &HFFFF&)
if segmentValue = 0 or farAddress <> expectedAddress then
    print "FAIL string_address value"
    end
end if
def seg = segmentValue
if peek(offsetValue) <> asc("A") or peek(offsetValue + 3) <> asc("D") then
    def seg
    print "FAIL string_address memory"
    end
end if
def seg
if sseg(nullValue) <> 0 then
    print "FAIL string_address null"
    end
end if
print "PASS string_address"
