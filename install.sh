#!/usr/bin/env sh
# Installe le binaire `note` et les complétions shell.
# Usage : ./install.sh
set -eu

cd "$(dirname "$0")"

echo "==> Compilation et installation du binaire"
cargo install --path . --force
BIN="${CARGO_HOME:-$HOME/.cargo}/bin/note"

echo "==> Complétions"
BASH_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions"
ZSH_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
mkdir -p "$BASH_DIR" "$ZSH_DIR"
"$BIN" completions bash > "$BASH_DIR/note"
"$BIN" completions zsh  > "$ZSH_DIR/_note"
echo "    bash : $BASH_DIR/note"
echo "    zsh  : $ZSH_DIR/_note"

cat <<MSG

==> Terminé. Binaire : $BIN

Vérifiez que ~/.cargo/bin est dans le PATH :
    command -v note

Pour zsh, si la complétion ne se déclenche pas, ajoutez dans ~/.zshrc AVANT compinit :
    fpath=($ZSH_DIR \$fpath)

Les notes seront écrites dans :
    $("$BIN" path)
(modifiable via la variable d'environnement NOTE_DIR)

Essayez :
    note take "ma première note" -c Divers -t essai
    note list
MSG
