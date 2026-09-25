declare sub showCommand ()

showCommand

sub showCommand
    dim commandLine as string
    commandLine = rtrim$(ltrim$(command$))
    print commandLine
end sub
