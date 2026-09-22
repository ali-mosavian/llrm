option explicit

function pick (byval flag as integer) as single
    if flag then
        pick = -1
    else
        pick = 0
    end if
end function
