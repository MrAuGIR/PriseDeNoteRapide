# note — prise de note rapide en ligne de commande

Capturer une information à la volée depuis un terminal déjà ouvert, sans quitter le clavier :

```bash
note take "Le client veut un export CSV, pas Akeneo" -c Projets -t export-csv,client
```

La note est horodatée automatiquement et ajoutée à un fichier JSONL. Elle peut ensuite être
exportée vers [frise-chronologique](../Rust/frise-chronologique).

## Installation

```bash
git clone <url> PriseNoteRapide && cd PriseNoteRapide
./install.sh
```

`install.sh` fait un `cargo install --path .` (binaire dans `~/.cargo/bin/note`) et installe les
complétions bash et zsh.

### Sur une autre machine (WSL2 Ubuntu, PC secondaire)

```bash
sudo apt install build-essential            # prérequis de compilation
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh   # si rustup absent
git clone <url> && cd PriseNoteRapide && ./install.sh
```

Ou directement, sans cloner :

```bash
cargo install --git <url>
```

### Sans Rust sur la machine cible

Compiler ici, copier là-bas :

```bash
cargo build --release
scp target/release/note machine:~/.local/bin/
```

Si les versions de glibc diffèrent entre les deux machines, viser plutôt une cible statique :

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
```

## Utilisation

### Prendre une note

```bash
note take "Réunion budget : arbitrage repoussé à octobre" -c Management -t budget,arbitrage
note take Pas besoin de guillemets si le texte est simple -c Divers
note take "Titre explicite" --title "Épi Saveur : format d'échange" -c Projets
note take "oubli d'hier" --at 2026-09-08T09:15 -c RH
journalctl -u nginx -n 20 | note take --stdin -c Infra -t incident
```

Options de `take` :

| Option | Effet |
|---|---|
| `-t, --tag <TAGS>` | Tags. Répétable **et** séparable par virgules : `-t a,b -t c` |
| `-c, --cat <CATÉGORIE>` | Catégorie — c'est elle qui détermine la ligne dans la frise |
| `--title <TITRE>` | Titre explicite (sinon dérivé de la première ligne) |
| `--at <QUAND>` | Antidater : `YYYY-MM-DD` ou `YYYY-MM-DDTHH:MM` |
| `--stdin` | Lire le corps sur l'entrée standard |

Les tags sont normalisés à l'écriture : minuscules, espaces en tirets, doublons supprimés,
accents conservés (`-t "Épi Saveur"` → `épi-saveur`).

### Relire

```bash
note list                        # tout, du plus ancien au plus récent
note list -n 10 --full           # les 10 dernières, texte complet
note list -c Projets --since 7d  # une catégorie, la dernière semaine
note search akeneo               # texte, titre, tags et catégorie
note show 3ff6ce74               # une note en entier (préfixe d'id, comme git)
note tags                        # tags utilisés, avec leur fréquence
note cats                        # catégories utilisées
note path                        # où sont stockées les notes
```

Filtres communs à `list`, `search` et `export` : `--since`, `--until`, `-t/--tag`, `-c/--cat`,
`-n/--limit`. Les dates acceptent `YYYY-MM-DD` ou un relatif : `today`, `yesterday`, `7d`, `2w`,
`3m`, `1y`.

### Modifier, supprimer

```bash
note edit 3ff6ce74                                  # ouvre $EDITOR sur le texte
note edit 3ff6ce74 --cat RH --add-tag urgent        # métadonnées seules, sans éditeur
note rm 3ff6ce74                                    # confirmation demandée
```

## Export vers la frise chronologique

```bash
note export --format frise --out ~/notes-frise.json
```

Puis dans frise-chronologique : **Fichier > Importer JSON…** et choisir le fichier.

Autres options :

```bash
note export --format frise --since 2026-09-01 --out ~/sept.json   # export incrémental
note export --format frise --group-by-day --out ~/frise.json      # 1 événement par jour+catégorie
note export --format md                                            # Markdown groupé par jour
note export --format json                                          # format interne brut
note export --format frise | jq '.[0]'                             # sans --out : sortie standard
```

### Deux pièges de la frise à connaître

1. **L'import ne dédoublonne pas.** Ré-importer un fichier déjà importé duplique tous ses
   événements. Pour alimenter la frise au fil de l'eau, exporter avec `--since` depuis la date du
   dernier import.
2. **Un `data.json` malformé est silencieusement écrasé.** La frise avale les erreurs de lecture
   et repart d'un store vide, qui écrase l'ancien à la sauvegarde suivante. Avant un import
   important :
   ```bash
   cp ~/.local/share/frise-chronologique/data.json ~/frise-data.bak.json
   ```

### Correspondance des champs

| Note | Champ frise |
|---|---|
| date de `created_at` | `date_start` = `date_end` (`YYYY-MM-DD`) |
| heure de `created_at` | `time_of_day` (`14h32`) et `date_label` (`09/09 14h32`) |
| `text` découpé en lignes | `content` (tableau de chaînes) |
| `title` | `title`, omis si absent → la frise dérive le titre |
| `category` | `category`, omise si absente → la frise applique `projet` |
| `tags` | `tags`, omis si vide |

L'axe vertical de la frise est construit **sur la catégorie seule** ; les tags n'apparaissent
qu'en infobulle et dans son export Markdown. C'est donc la catégorie qu'il faut soigner.

## Stockage

Un fichier JSONL, une note par ligne, en ajout seul :

```
~/.local/share/prise-note-rapide/notes.jsonl
```

Modifiable via `NOTE_DIR` (par exemple pour placer les notes dans un dossier déjà synchronisé) :

```bash
export NOTE_DIR="$HOME/Sync/notes"
```

Une ligne du fichier :

```json
{"id":"9284378b-...","created_at":"2026-09-09T00:29:30.018+02:00","text":"ligne A\nligne B","category":"Projets","tags":["épi-saveur"]}
```

L'ajout est une écriture unique en `O_APPEND` : deux terminaux peuvent prendre une note en même
temps sans risque d'entremêlement. Les suppressions et modifications réécrivent le fichier via un
temporaire puis un `rename` atomique. Une ligne corrompue est signalée sur `stderr` et ignorée,
sans faire perdre le reste du fichier.

## Développement

```bash
cargo test          # dont les tests qui verrouillent la compatibilité du format frise
cargo clippy --all-targets
```

Le module `src/export/frise.rs` contient, dans ses tests, une **copie conforme** de la struct
`ExternalEvent` de `frise-chronologique/src/storage/import.rs`. C'est le seul point de couplage
entre les deux outils : si le format d'import de la frise change, resynchroniser cette copie.
