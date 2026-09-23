on error goto handler
dim i as integer, limit as integer
limit = 2
for i = 0 to limit
print i
next i
end
handler:
resume next
