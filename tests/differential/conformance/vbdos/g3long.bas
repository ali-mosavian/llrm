option explicit

declare function mix_long (byval left_value as long, byval middle_value as long, byval right_value as long) as long

dim answer_value as long
dim left_value as long
dim middle_value as long
dim right_value as long

read left_value, middle_value, right_value
answer_value = mix_long(left_value, middle_value, right_value)
if answer_value <> 2577 then
    print "FAIL g3_byval_long arithmetic"
    end
end if
if left_value <> 1000 or middle_value <> 777 or right_value <> 1234 then
    print "FAIL g3_byval_long byval"
    end
end if

print "PASS g3_byval_long"

data 1000, 777, 1234

function mix_long (byval left_value as long, byval middle_value as long, byval right_value as long) as long
    left_value = left_value * 3
    middle_value = middle_value \ 7
    right_value = right_value mod 13
    mix_long = left_value - middle_value * 5 + right_value * 11
end function
