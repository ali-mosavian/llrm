' Focused string extraction and construction witnesses.
dim sourceText as string

read sourceText
if left$(sourceText, 3) <> "ABC" then
    print "FAIL stringparts left"
    end
end if
if mid$(sourceText, 3, 3) <> "CDE" then
    print "FAIL stringparts mid"
    end
end if
if right$(sourceText, 2) <> "FG" then
    print "FAIL stringparts right"
    end
end if
if instr(2, sourceText, "CD") <> 3 then
    print "FAIL stringparts instr"
    end
end if
if space$(2) + string$(3, "x") <> "  xxx" then
    print "FAIL stringparts build"
    end
end if
if left$(sourceText, 0) <> "" then
    print "FAIL stringparts left-zero"
    end
end if
if mid$(sourceText, 8) <> "" then
    print "FAIL stringparts mid-past-end"
    end
end if
if right$(sourceText, 0) <> "" then
    print "FAIL stringparts right-zero"
    end
end if
if instr(8, sourceText, "A") <> 0 then
    print "FAIL stringparts instr-past-end"
    end
end if
if space$(0) <> "" then
    print "FAIL stringparts space-zero"
    end
end if
if string$(0, "x") <> "" then
    print "FAIL stringparts string-zero"
    end
end if
print "PASS stringparts"
end

data "ABCDEFG"
