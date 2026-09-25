' COMMAND$ preserves the exact DOS argument text supplied by the runner.
dim commandText as string

commandText = command$
if commandText <> "ALPHA BETA42" then
    print "FAIL command text="; commandText
    end
end if
if len(commandText) <> 12 then
    print "FAIL command length"
    end
end if
print "PASS command"
end
