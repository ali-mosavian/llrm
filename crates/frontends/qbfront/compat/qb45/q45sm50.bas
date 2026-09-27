' MID$ assignment and fixed-string padding have independent results.
dim dynamicText as string
dim fixedText as string * 5

read dynamicText
mid$(dynamicText, 2, 3) = "XYZ"
if dynamicText <> "aXYZef" then
    print "FAIL stringmut mid"
    end
end if
fixedText = "Q"
if fixedText <> "Q    " or len(fixedText) <> 5 then
    print "FAIL stringmut fixed"
    end
end if
if dynamicText + left$(fixedText, 1) <> "aXYZefQ" then
    print "FAIL stringmut concat"
    end
end if
print "PASS stringmut"
end

data "abcdef"
