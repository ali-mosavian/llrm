option explicit

declare sub recover_locally ()

dim shared recovery_count as integer

call recover_locally
if recovery_count = 1 then
    print "PASS local_error"
else
    print "FAIL local_error return"
end if
end

sub recover_locally ()
    dim recovered_here as integer

    on local error goto caughtError
100 error 53
    print "FAIL local_error missed"
    end

caughtError:
    if err <> 53 then
        print "FAIL local_error errnum"
        end
    end if
    if erl <> 100 then
        print "FAIL local_error errline"
        end
    end if
    recovered_here = -1
    resume 200

200 ' recovery target after the handler
    if recovered_here = 0 then
        print "FAIL local_error exact"
        end
    end if
    recovery_count = recovery_count + 1
end sub
