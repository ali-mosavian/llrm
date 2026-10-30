# landing_busy: true while a land.sh process runs. Matches the process, not any command line that names land.sh (a session's watcher loop did, and held the runner idle for 80 minutes).
landing_busy() { ${LANDING_PS:-ps -eo args} | awk '{for(i=1;i<=2;i++) if ($i ~ /(^|\/)land\.sh$/) f=1} END{exit !f}'; }
# LANDING_PS: the command that lists command lines; a test injects a fixed table.
