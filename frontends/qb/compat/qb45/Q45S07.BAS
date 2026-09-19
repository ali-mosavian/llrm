' QB45 compatibility source.
dim fixedText as string * 8
dim text as string
dim passed as integer

fixedText = "cat"
text = ltrim$("  ") + fixedText + "!"
mid$(text, 1, 3) = "dog"
passed = left$(text, 4) = "dog " and right$(text, 1) = "!" and len(fixedText) = 8

if passed then
    print "PASS strings"
else
    print "FAIL strings mutation"
end if
end
