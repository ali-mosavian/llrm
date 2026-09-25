declare sub consume alias "CONSUME" (seg buffer as any)

sub probe ()
    dim index as integer
    dim words(0 to 31) as integer

    index = 1
    consume words(index)
end sub
