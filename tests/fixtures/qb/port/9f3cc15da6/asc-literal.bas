option explicit

declare function firstCode ( text as string ) as integer

dim code as integer

code = firstCode("m")
print code

function firstCode ( text as string ) as integer
    if len(text) = 0 then
        firstCode = -1
        exit function
    end if
    firstCode = asc(text)
end function
