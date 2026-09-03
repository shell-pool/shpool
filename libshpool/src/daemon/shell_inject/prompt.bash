# In Bash 5.1+, PROMPT_COMMAND supports arrays. However, older
# versions of Bash (such as Bash 3.2, the default system shell on
# macOS) only execute PROMPT_COMMAND if it is a scalar string;
# assigning an array causes Bash 3.2 to silently ignore it.
# We capture any existing hooks (array or scalar) into
# SHPOOL__OLD_PROMPT_COMMAND, unset PROMPT_COMMAND, and assign
# PROMPT_COMMAND as a scalar string to ensure universal
# compatibility.
if [ -n "${PROMPT_COMMAND+x}" ]; then
  SHPOOL__OLD_PROMPT_COMMAND=("${PROMPT_COMMAND[@]}")
else
  SHPOOL__OLD_PROMPT_COMMAND=()
fi
SHPOOL__OLD_PS1="${PS1:-}"
function __shpool__prompt_command() {
  local ret=$? ps="${PIPESTATUS[*]}"
  local -a pipestatus
  pipestatus=($ps)
  local env_file="${SHPOOL_SESSION_DIR}/forward.env"
  local stamp_file="${env_file}.stamp"
  if [ -n "${SHPOOL_SESSION_DIR}" ] && [ -f "${env_file}" ]; then
    if [ ! -f "${stamp_file}" ] || [ "${env_file}" -nt "${stamp_file}" ]; then
      touch -r "${env_file}" "${stamp_file}" 2>/dev/null

      local allexport_was_set=0
      case "$-" in
        *a*) allexport_was_set=1 ;;
      esac
      set -a
      . "${env_file}"
      if [ "$allexport_was_set" -eq 0 ] ; then
        set +a
      fi
    fi
  fi

  PS1="${SHPOOL__OLD_PS1}"
  local restore_cmd=""
  for status in "${pipestatus[@]}"; do
    if [ -n "$restore_cmd" ]; then
      restore_cmd="$restore_cmd | (exit $status)"
    else
      restore_cmd="(exit $status)"
    fi
  done
  if [ -z "$restore_cmd" ]; then
    restore_cmd="(exit $ret)"
  fi
  for prompt_hook in "${SHPOOL__OLD_PROMPT_COMMAND[@]}"
  do
    eval "$restore_cmd; eval \"${prompt_hook}\""
  done
  PS1="@@PROMPT_PREFIX@@${PS1}"
  return $ret
}
unset PROMPT_COMMAND
unset PIPESTATUS
PROMPT_COMMAND=__shpool__prompt_command
