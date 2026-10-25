declare sub showCommand ()

showCommand

sub showCommand
    dim commandLine as string
    commandLine = rtrim$(ltrim$(command$))
    if commandLine = "" then
        print "EMPTY"
    else
        print commandLine
    end if
end sub
