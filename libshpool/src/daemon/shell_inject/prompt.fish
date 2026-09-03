# Fish only added the `-nt` (newer-than) binary operator to its
# builtin `test` in fish 4.0b1. In older fish versions (such as
# fish 3.x), calling builtin `test -nt` errors with "unexpected
# argument". To maintain zero-fork prompt evaluation on
# fish 4+ while remaining compatible with fish 3, we
# probe for `-nt` support once at injection
# time and define `__shpool_is_newer` to use the builtin if
# available, falling back to `command test` (coreutils)
# otherwise.
functions --copy fish_prompt shpool__old_prompt
function __shpool_set_status; return $argv[1]; end
set -l __shpool_nt_err (test /dev/null -nt /dev/null 2>&1)
if test -z "$__shpool_nt_err"
    function __shpool_is_newer; test $argv[1] -nt $argv[2]; end
else
    function __shpool_is_newer; command test $argv[1] -nt $argv[2]; end
end
function fish_prompt
    set -l last_status $status
    set -l env_file "$SHPOOL_SESSION_DIR/forward.env"
    set -l stamp_file "$env_file.stamp"
    if test -n "$SHPOOL_SESSION_DIR"; and test -f "$env_file"
        if test ! -f "$stamp_file"; or __shpool_is_newer "$env_file" "$stamp_file"
            touch -r "$env_file" "$stamp_file" 2>/dev/null
            source "$env_file"
        end
    end
    echo -n "@@PROMPT_PREFIX@@"
    __shpool_set_status $last_status
    shpool__old_prompt
end
