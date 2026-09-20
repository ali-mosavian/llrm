option explicit

declare function compat_external cdecl alias "compat_external" (byval source_value as long) as long

dim answer_value as long

answer_value = compat_external(41)
if answer_value <> 42 then
    print "FAIL cdecl_alias_external result"
    end
end if

print "PASS cdecl_alias_external"
