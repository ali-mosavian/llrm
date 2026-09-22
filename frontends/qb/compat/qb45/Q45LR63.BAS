' LSET and RSET justify and truncate against fixed and dynamic field widths.
dim sourceText as string
dim longText as string
dim leftFixed as string * 6
dim rightFixed as string * 6
dim truncated as string * 4
dim dynamicText as string

read sourceText, longText
lset leftFixed = sourceText
rset rightFixed = sourceText
lset truncated = longText
dynamicText = space$(7)
rset dynamicText = sourceText

if leftFixed <> "AB    " then
    print "FAIL justify lset"
    end
end if
if rightFixed <> "    AB" then
    print "FAIL justify rset"
    end
end if
if truncated <> "WXYZ" then
    print "FAIL justify truncate"
    end
end if
if dynamicText <> "     AB" then
    print "FAIL justify dynamic"
    end
end if
print "PASS justify"
end

data "AB", "WXYZ123"
