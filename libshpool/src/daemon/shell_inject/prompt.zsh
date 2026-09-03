typeset -a precmd_functions
SHPOOL__OLD_PROMPT="${PROMPT}"
function __shpool__reset_rprompt() {
  local ret=$?
  local env_file="${SHPOOL_SESSION_DIR:-}/forward.env"
  local stamp_file="${env_file}.stamp"
  if [ -n "${SHPOOL_SESSION_DIR:-}" ] && [ -f "${env_file}" ]; then
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

  PROMPT="${SHPOOL__OLD_PROMPT}"
  return $ret
}
precmd_functions[1,0]=(__shpool__reset_rprompt)
function __shpool__prompt_command() {
  local ret=$?
  PROMPT="@@PROMPT_PREFIX@@${PROMPT}"
  return $ret
}
precmd_functions+=(__shpool__prompt_command)
