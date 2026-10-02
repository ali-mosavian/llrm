' Requires /Fs. Together the payloads exceed the near heap.
dim leftText as string
dim rightText as string
dim count as integer
dim passed as integer

count = 30000
leftText = string$(count, "x")
rightText = string$(count, "y")
passed = asc(mid$(leftText, count, 1)) = 120
passed = passed and asc(mid$(rightText, count, 1)) = 121

if passed then
    print "PASS pds-far-string"
else
    print "FAIL pds-far-string payload"
end if
end
