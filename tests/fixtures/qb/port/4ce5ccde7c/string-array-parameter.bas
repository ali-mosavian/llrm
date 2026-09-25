declare sub appendOne (strm() as string, i as integer)

dim i as integer

redim strm(0 to 1) as string
strm(0) = "A"
i = 0
appendOne strm(), i
print strm(0)

sub appendOne (strm() as string, i as integer)
    dim oneChar as string * 1

    oneChar = "B"
    strm(i) = strm(i) + oneChar
end sub
