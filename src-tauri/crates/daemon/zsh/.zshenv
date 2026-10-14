if [[ -n "${SPAWNSHELL_ZSH_ZDOTDIR+X}" ]]; then
  ZDOTDIR=$SPAWNSHELL_ZSH_ZDOTDIR
else
  unset ZDOTDIR
fi

{
  [[ -f "${ZDOTDIR:-$HOME}/.zshenv" ]] && builtin source "${ZDOTDIR:-$HOME}/.zshenv"
} always {
  if [[ -o interactive ]]; then
    builtin autoload -Uz add-zsh-hook
    _spawnshell_command_start() { builtin print -n '\e]133;C\a' }
    _spawnshell_command_end() { builtin print -n '\e]133;D\a' }
    add-zsh-hook preexec _spawnshell_command_start
    add-zsh-hook precmd _spawnshell_command_end
  fi
}
