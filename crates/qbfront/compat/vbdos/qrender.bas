option explicit

'$static
type Frame_State
    frame_count as long
    map_name as string * 8
end type
'$dynamic

declare sub advance_frame ( _
    state as Frame_State, _
    byval step_count as integer _
)

dim frame_state as Frame_State
dim step_count as integer

frame_state.frame_count = 40
frame_state.map_name = "QRENDER"
step_count = 2
call advance_frame(frame_state, step_count)

if frame_state.frame_count <> 42 then
    print "FAIL qrender_shape frame_count"
    end
end if
if frame_state.map_name <> "QRENDER " then
    print "FAIL qrender_shape fixed_string"
    end
end if
if step_count <> 2 then
    print "FAIL qrender_shape byval"
    end
end if

print "PASS qrender_shape"

sub advance_frame ( _
    state as Frame_State, _
    byval step_count as integer _
)
    state.frame_count = state.frame_count + step_count
    step_count = 99
end sub
