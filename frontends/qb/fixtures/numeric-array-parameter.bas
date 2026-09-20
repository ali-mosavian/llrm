option explicit

declare sub setFirst (values() as long)

'$dynamic
dim values(1 to 2) as long
setFirst values()

sub setFirst (values() as long)
    values(1) = 42
end sub
