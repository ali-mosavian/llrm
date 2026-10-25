declare sub copyFirst ()

copyFirst

sub copyFirst
    dim arguments(16) as string
    dim destination as string * 64
    arguments(0) = "HELLO"
    destination = arguments(0)
    print destination
end sub
