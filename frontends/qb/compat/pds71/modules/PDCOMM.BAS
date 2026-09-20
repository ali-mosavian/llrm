declare sub changeValue (leftValue as long, rightValue as long)

common shared /memoryPool/ value&

value& = 10
changeValue 20, 12
if value& = 42 then
    print "PASS pds-common-modules"
else
    print "FAIL pds-common-modules total"
end if
end
