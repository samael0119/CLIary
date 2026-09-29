# CLIary records only executable names; command arguments are never persisted.
function __cliary_fish_preexec --on-event fish_preexec
    set -l line (string trim -- $argv[1])
    set -l executable (string split -m1 ' ' -- $line)[1]
    string match -qr '^[A-Za-z0-9_./+-]+$' -- $executable; or return
    command -sq -- $executable; or return
    command @CLIARY_BIN@ internal record $executable >/dev/null 2>&1 &
end
