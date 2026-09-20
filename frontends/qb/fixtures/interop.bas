dim answer as long
answer = twice&(21)
call report(answer)

function Twice& (byval value as long)
    Twice& = value + value
end function

sub Report (byval value as long)
    print value
end sub
