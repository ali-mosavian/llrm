' Requires /E /X. ON LOCAL ERROR must not leak into the caller.
declare sub missingFile ()
declare sub reportFail (checkName as string)
dim shared recoveryCount as integer

missingFile
if recoveryCount = 1 then
    print "PASS pds-local-error"
else
    call reportFail("return")
end if
end

sub missingFile
    dim handled as integer

    on local error goto missing
100 error 53
    call reportFail("no-error")
    exit sub

missing:
    if err <> 53 then
        call reportFail("errnum")
        exit sub
    end if
    if erl <> 100 then
        call reportFail("errline")
        exit sub
    end if
    handled = -1
    resume recovered

recovered:
    if not handled then
        call reportFail("err")
        exit sub
    end if
    recoveryCount = recoveryCount + 1
    exit sub
end sub

sub reportFail (checkName as string)
    print "FAIL pds-local-error "; checkName
end sub
