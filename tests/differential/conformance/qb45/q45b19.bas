' QB45 compatibility source.
dim sourceText as string * 4
dim targetText as string * 4

sourceText = "BLOB"
def seg = varseg(sourceText)
bsave "Q45M19.BSV", varptr(sourceText), 4
def seg = varseg(targetText)
bload "Q45M19.BSV", varptr(targetText)
def seg

if targetText = "BLOB" then
    print "PASS bload"
else
    print "FAIL bload bytes"
end if
end
