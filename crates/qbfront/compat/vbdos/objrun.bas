option explicit

sub exerciseObject (sourceControl as control)
    if typeof sourceControl is CommandButton then
        sourceControl.Enabled = 0
    end if
end sub

sub Form_Load ()
    load frmSecondary
    frmSecondary.Show
    doevents
    unload frmSecondary
end sub
