option explicit

declare sub combine (values() as long)
declare sub inspect (values() as long)

'$dynamic
dim values(1 to 3) as long
combine values()

sub combine (values() as long)
    values(1) = values(2) + values(3)
    inspect values()
    values(1) = values(2) + values(3)
end sub

sub inspect (values() as long)
end sub
