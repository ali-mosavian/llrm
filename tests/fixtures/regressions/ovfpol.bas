dim value as integer
on error goto handler
value = 32767
value = value + 1
print value
print "DONE"
end
handler:
print "ERR"; err
print "DONE"
end
