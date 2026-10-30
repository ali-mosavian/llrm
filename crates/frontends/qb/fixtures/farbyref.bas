defint a-z
type Item
    value as integer
end type
declare sub Work ()
declare sub Touch (value as integer)
Work
end
sub Work
    dim items(1 to 2) as Item
    items(1).value = 5
    Touch items(1).value
    print items(1).value
end sub
sub Touch (value as integer)
    value = value + 1
end sub
