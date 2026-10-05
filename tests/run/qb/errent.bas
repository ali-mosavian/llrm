' The registered handler is entered by the runtime, never by falling through END.
dim caught as integer
dim numerator as long, divisor as long, result as long
read numerator, divisor
on error goto handler
result = numerator \ divisor
print caught
print "DONE"
end

handler:
caught = err
resume next
data 7, 0
