# CLIary records only the executable name. This function never writes a command line.
__cliary_last_history=0
__cliary_initial_entry=$(HISTTIMEFORMAT= builtin history 1)
if [[ $__cliary_initial_entry =~ ^[[:space:]]*([0-9]+)[[:space:]]+ ]]; then
  __cliary_last_history=${BASH_REMATCH[1]}
fi
unset __cliary_initial_entry
__cliary_prompt() {
  local entry number command_line executable kind
  entry=$(HISTTIMEFORMAT= builtin history 1) || return 0
  if [[ $entry =~ ^[[:space:]]*([0-9]+)[[:space:]]+(.*)$ ]]; then
    number=${BASH_REMATCH[1]}
    command_line=${BASH_REMATCH[2]}
  else
    return 0
  fi
  if [[ $number == "$__cliary_last_history" ]]; then return 0; fi
  __cliary_last_history=$number
  executable=${command_line%%[[:space:]]*}
  [[ $executable =~ ^[a-zA-Z0-9_./+-]+$ ]] || return 0
  kind=$(type -t -- "$executable" 2>/dev/null) || return 0
  [[ $kind == file ]] || return 0
  command @CLIARY_BIN@ internal record "$executable" >/dev/null 2>&1 &
}
case $(declare -p PROMPT_COMMAND 2>/dev/null) in
  'declare -a'*)
    __cliary_present=0
    for __cliary_item in "${PROMPT_COMMAND[@]}"; do
      [[ $__cliary_item == __cliary_prompt ]] && __cliary_present=1
    done
    [[ $__cliary_present == 1 ]] || PROMPT_COMMAND+=(__cliary_prompt)
    unset __cliary_present __cliary_item
    ;;
  *)
    case ";${PROMPT_COMMAND:-};" in
      *';__cliary_prompt;'*) ;;
      *) PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND;}__cliary_prompt" ;;
    esac
    ;;
esac
