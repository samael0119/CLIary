# Zsh supplies the expanded command as argument 3, including its actual aliases.
# Only two executable names cross the process boundary; never the command line.
autoload -Uz add-zsh-hook
__cliary_zsh_preexec() {
  emulate -L zsh
  local entered executable
  local -a original expanded
  original=(${(z)1})
  expanded=(${(z)${3:-$1}})
  entered=${(Q)original[1]}
  executable=${(Q)expanded[1]}
  [[ $entered =~ '^[A-Za-z0-9_./+-]+$' && $executable =~ '^[A-Za-z0-9_./+-]+$' ]] || return 0
  # A function can shadow a file in PATH; it does not establish a tool identity.
  [[ $(whence -w -- "$executable" 2>/dev/null) == "$executable: command" ]] || return 0
  command @CLIARY_BIN@ internal record "$entered" --resolved-executable "$executable" >/dev/null 2>&1 &!
}
add-zsh-hook preexec __cliary_zsh_preexec
