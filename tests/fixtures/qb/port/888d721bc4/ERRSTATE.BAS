on error goto first
print "armed first"
on error goto second
print "armed second"
on error goto 0
error 11
end
first:
resume next
second:
resume next
