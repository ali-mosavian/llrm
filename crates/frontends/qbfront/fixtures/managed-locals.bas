declare sub showCommand ()

showCommand

sub showCommand
    dim arguments(16) as string
    dim commandLine as string
    commandLine = command$
    print commandLine
end sub
