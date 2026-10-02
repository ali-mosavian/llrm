' FILEATTR reports each open mode and a distinct DOS handle.
dim inputHandle as integer
dim outputHandle as integer
dim randomHandle as integer
dim appendHandle as integer
dim binaryHandle as integer

open "FA64I.DAT" for output as #1
print #1, "I";
close #1

open "FA64I.DAT" for input as #1
open "FA64O.DAT" for output as #2
open "FA64R.DAT" for random as #3 len = 2
open "FA64A.DAT" for append as #4
open "FA64B.DAT" for binary as #5

if fileattr(1, 1) <> 1 then
    print "FAIL fileattr input"
    end
end if
if fileattr(2, 1) <> 2 then
    print "FAIL fileattr output"
    end
end if
if fileattr(3, 1) <> 4 then
    print "FAIL fileattr random"
    end
end if
if fileattr(4, 1) <> 8 then
    print "FAIL fileattr append"
    end
end if
if fileattr(5, 1) <> 32 then
    print "FAIL fileattr binary"
    end
end if

inputHandle = fileattr(1, 2)
outputHandle = fileattr(2, 2)
randomHandle = fileattr(3, 2)
appendHandle = fileattr(4, 2)
binaryHandle = fileattr(5, 2)
if inputHandle < 5 or outputHandle < 5 or randomHandle < 5 then
    print "FAIL fileattr low-handle"
    end
end if
if appendHandle < 5 or binaryHandle < 5 then
    print "FAIL fileattr low-handle"
    end
end if
if inputHandle = outputHandle or inputHandle = randomHandle then
    print "FAIL fileattr duplicate"
    end
end if
if outputHandle = randomHandle or outputHandle = appendHandle then
    print "FAIL fileattr duplicate"
    end
end if
if randomHandle = appendHandle or randomHandle = binaryHandle then
    print "FAIL fileattr duplicate"
    end
end if
if appendHandle = binaryHandle or inputHandle = binaryHandle then
    print "FAIL fileattr duplicate"
    end
end if
if inputHandle = appendHandle or outputHandle = binaryHandle then
    print "FAIL fileattr duplicate"
    end
end if

close
kill "FA64I.DAT"
kill "FA64O.DAT"
kill "FA64R.DAT"
kill "FA64A.DAT"
kill "FA64B.DAT"
print "PASS fileattr"
end
