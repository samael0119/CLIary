# CLIary records only the first external executable of an interactive command.
autoload -Uz add-zsh-hook
__cliary_zsh_preexec() {
  local executable
  local -a words
  words=(${(z)1})
  executable=${words[1]}
  [[ $executable =~ '^[A-Za-z0-9_./+-]+$' ]] || return 0
  whence -p -- "$executable" >/dev/null 2>&1 || return 0
  command @CLIARY_BIN@ internal record "$executable" >/dev/null 2>&1 &!
}
add-zsh-hook preexec __cliary_zsh_preexec
