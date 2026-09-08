# Plan — `note` : outil CLI de prise de note rapide

## Contexte

Besoin : capturer une information à la volée depuis un terminal déjà ouvert, sans quitter le
clavier ni ouvrir d'application. Une seule commande (`note take "..."`), horodatée
automatiquement, avec tags et catégorie optionnels.

Les notes doivent pouvoir alimenter la frise chronologique existante
(`~/projects/Rust/frise-chronologique`), et l'outil doit s'installer facilement sur d'autres
machines (notamment le WSL2 Ubuntu du PC secondaire).

Décisions validées : **Rust** (cohérent avec le stack existant, binaire unique installable via
`cargo install`), stockage **JSONL append-only**, **pas de synchronisation** entre machines pour
l'instant, intégration frise via une commande **`note export --format frise`** (découplage : le
format interne reste riche, la frise reçoit exactement ce qu'elle sait lire).

---

## Contrainte n°1 : le format attendu par la frise

Vérifié dans `~/projects/Rust/frise-chronologique/src/storage/import.rs`. C'est un **tableau JSON
nu** (pas d'objet racine, pas de champ `version`), et la struct de désérialisation est :

```rust
pub struct ExternalEvent {
    #[serde(default)] pub title: Option<String>,
    #[serde(default)] pub category: Option<String>,
    #[serde(default)] pub tags: Option<Vec<String>>,
    #[serde(default)] pub date_label: Option<String>,
    pub date_start: NaiveDate,                        // SEUL champ obligatoire
    #[serde(default)] pub date_end: Option<NaiveDate>,
    #[serde(default)] pub is_range: bool,             // toléré mais ignoré (recalculé)
    #[serde(default)] pub time_of_day: Option<String>,
    #[serde(default)] pub content: Vec<String>,       // lignes, jointes par "\n" à l'import
}
```

Points à respecter impérativement :

- Les dates sont des `chrono::NaiveDate` → chaîne **`"YYYY-MM-DD"` stricte**. Un RFC3339 avec
  heure/timezone **fait échouer l'import**.
- `content` est un **tableau de chaînes**, pas une chaîne.
- `time_of_day` et `date_label` sont des chaînes libres non parsées (l'existant utilise `"14h32"`
  et `"08/06 14h32"`).
- Pas de champ `id` ni `color` à l'import : la frise génère un UUID et déduit la couleur de la
  catégorie.
- Pas de `deny_unknown_fields` → les clés en trop sont ignorées silencieusement.
- L'axe vertical de la frise est construit **sur `category` uniquement** ; `tags` ne sert qu'aux
  infobulles et à l'export Markdown. Donc : soigner la catégorie, les tags sont du confort.
- Import GUI seulement : menu `Fichier > Importer JSON…`, pas de CLI côté frise.

⚠️ **Deux pièges de la frise à documenter dans le README** :
1. L'import **ajoute sans dédoublonnage** (`data.events.push`) → ré-importer le même fichier
   duplique tout. D'où l'option `--since` pour les exports incrémentaux.
2. `JsonStore::load()` avale toutes les erreurs : un `~/.local/share/frise-chronologique/data.json`
   illisible ou malformé est silencieusement remplacé par un store vide, écrasé au save suivant.
   → conseiller une sauvegarde avant import.

---

## Architecture

Projet : `/home/mraugir/projects/PriseNoteRapide`, binaire nommé **`note`**.

```
PriseNoteRapide/
├── Cargo.toml
├── README.md
├── install.sh
├── .gitignore                 # /target
└── src/
    ├── main.rs                # parsing clap + dispatch
    ├── cli.rs                 # définition des commandes/options (clap derive)
    ├── model.rs               # struct Note
    ├── store.rs               # lecture/écriture du JSONL, résolution d'id
    ├── filter.rs              # filtres partagés (--since/--until/--tag/--cat/--limit)
    └── export/
        ├── mod.rs
        ├── frise.rs           # ExternalEvent + mapping Note -> frise
        └── markdown.rs
```

Dépendances (versions alignées sur celles de la frise pour rester homogène) :

```toml
clap       = { version = "4", features = ["derive"] }
clap_complete = "4"
serde      = { version = "1", features = ["derive"] }
serde_json = "1"
chrono     = { version = "0.4", features = ["serde"] }
uuid       = { version = "1", features = ["v4", "serde"] }
dirs       = "6"
anyhow     = "1"
```

### Modèle interne (`src/model.rs`)

```rust
#[derive(Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub created_at: DateTime<FixedOffset>,   // Local::now().fixed_offset(), RFC3339 complet
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")] pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]   pub tags: Vec<String>,
}
```

`FixedOffset` plutôt que `Local` pour un aller-retour sérialisation fidèle même si la machine
change de fuseau. L'horodatage interne garde l'heure complète ; c'est l'export qui dégrade vers
la date nue exigée par la frise.

### Stockage (`src/store.rs`)

- Chemin : `$NOTE_DIR` si défini, sinon `dirs::data_local_dir()/prise-note-rapide/notes.jsonl`
  → `~/.local/share/prise-note-rapide/notes.jsonl`. `create_dir_all` à la première écriture.
- Ajout : `OpenOptions::new().create(true).append(true)`, une ligne JSON + `\n` en un seul
  `write_all`. Sur Linux, une écriture courte en `O_APPEND` est atomique → pas de corruption même
  avec deux terminaux ouverts.
- Lecture : `BufReader::lines()`, une ligne illisible est **signalée sur stderr et sautée**, jamais
  fatale (contrairement à la frise, on ne perd rien silencieusement).
- Modification/suppression : réécriture complète dans un fichier temporaire du même répertoire
  puis `rename` atomique.
- Résolution d'identifiant par **préfixe court d'UUID** (à la git, 8 caractères affichés) ;
  erreur explicite si le préfixe est ambigu.

---

## Commandes

| Commande | Description |
|---|---|
| `note take <texte…>` | Ajoute une note horodatée. Alias : `add`, `t`. |
| `note list` | Liste les notes, plus récentes en premier. |
| `note search <motif>` | Recherche insensible à la casse dans texte, titre et tags. |
| `note show <id>` | Affiche une note complète. |
| `note edit <id>` | Édite le texte (`$EDITOR`) ou remplace les tags/catégorie via options. |
| `note rm <id>` | Supprime (avec confirmation, `-f` pour forcer). |
| `note tags` / `note cats` | Liste les tags / catégories existants avec leur nombre d'occurrences. |
| `note export` | Export filtré (voir plus bas). |
| `note path` | Affiche le chemin du fichier de données. |
| `note completions <shell>` | Génère la complétion bash/zsh. |

**Options de `take`** :
`-t, --tag <TAGS>` (répétable **et** acceptant une liste séparée par des virgules :
`-t a,b -t c`), `-c, --cat <CATÉGORIE>`, `--title <TITRE>`,
`--at <YYYY-MM-DD[THH:MM]>` pour antidater, `--stdin` pour lire le corps sur l'entrée standard
(`cmd | note take --stdin -c infra`).

Normalisation des tags à l'écriture : `trim`, minuscules, espaces → `-`, doublons supprimés —
pour rester cohérent avec la convention déjà présente dans `notes.enriched.json`
(`épi-saveur`, `conflit-d-intérêt`). Les accents sont **conservés**.

**Filtres partagés** par `list`, `search` et `export` (`src/filter.rs`) :
`--since <date>`, `--until <date>`, `--tag <t>`, `--cat <c>`, `--limit <n>`.
`--since` accepte une date absolue `YYYY-MM-DD` ou un raccourci relatif (`7d`, `2w`, `today`).

---

## Export vers la frise (`src/export/frise.rs`) — cœur du sujet

Mapping d'une `Note` vers un `ExternalEvent` :

| Champ frise | Valeur produite |
|---|---|
| `date_start` | `created_at.date_naive()`, format `%Y-%m-%d` |
| `date_end` | identique à `date_start` |
| `is_range` | `false` |
| `time_of_day` | `created_at.format("%Hh%M")` → `"14h32"` |
| `date_label` | `created_at.format("%d/%m %Hh%M")` → `"09/09 14h32"` |
| `content` | `text.split('\n')` (lignes vides conservées telles quelles hors bords) |
| `title` | `note.title` si présent, sinon **omis** → la frise dérive le titre du `date_label` + 1ʳᵉ ligne |
| `category` | `note.category` si présent, sinon **omis** → la frise applique `"projet"` |
| `tags` | `note.tags`, omis si vide |

`--group-by-day` (optionnel) : regroupe les notes par `(date, catégorie)` en un seul événement —
`content` = concaténation des lignes de chaque note, `tags` = union, `date_label` = `"%d/%m"`,
`time_of_day` = `null`. C'est exactement la forme du `notes.json` déjà présent dans le dépôt de la
frise, utile pour une frise moins dense.

Formats supportés : `--format frise` (défaut), `json` (format interne brut), `md`.
`--out <FICHIER>` sinon stdout, ce qui permet `note export --format frise | jq`.

Usage type :
```bash
note export --format frise --since 2026-09-01 --out ~/notes-frise.json
# puis dans la frise : Fichier > Importer JSON… > ~/notes-frise.json
```

---

## Installation et portabilité

- **Machine principale** : `cargo install --path .` → `~/.cargo/bin/note`, déjà dans le `PATH`
  (bash et zsh).
- **WSL2 Ubuntu du PC secondaire** : `git clone` puis `cargo install --path .`, ou
  `cargo install --git <url>` une fois le dépôt poussé. Prérequis WSL2 : rustup +
  `build-essential`.
- **Sans Rust sur la machine cible** : `cargo build --release` puis copie de
  `target/release/note` vers `~/.local/bin/`. Ajouter une note README sur la cible
  `x86_64-unknown-linux-musl` pour un binaire sans dépendance glibc.
- `install.sh` : `cargo install --path .` + installation des complétions
  (`~/.local/share/bash-completion/completions/note`, `~/.local/share/zsh/site-functions/_note`)
  + rappel du `NOTE_DIR`.
- `NOTE_DIR` permet de pointer ailleurs (utile si un jour tu mets le dossier dans un espace
  synchronisé — la synchro est hors périmètre pour cette version).

---

## Étapes de mise en place

1. **Squelette** — `cargo init --name prise-note-rapide` dans `/home/mraugir/projects/PriseNoteRapide`,
   `[[bin]] name = "note"`, `.gitignore`, `git init`. Vérifier `cargo build`.
2. **Modèle + stockage** — `model.rs` et `store.rs` : `append`, `read_all`, `rewrite`,
   `resolve_id_prefix`, résolution du chemin de données. Tests unitaires sur fichier temporaire.
3. **`note take`** — CLI clap minimale, normalisation des tags, `--at`, `--stdin`. Premier
   jalon utilisable au quotidien.
4. **Lecture** — `list`, `show`, `search`, `tags`, `cats` + `filter.rs`. Sortie terminal lisible
   (id court, date, catégorie, tags, 1ʳᵉ ligne), `--json` pour le scriptable.
5. **Édition** — `edit`, `rm` via réécriture temp + `rename`.
6. **Export frise** — `export/frise.rs`, mapping ci-dessus, `--group-by-day`, `md`.
7. **Compatibilité verrouillée par un test** — dans `export/frise.rs`, un module `#[cfg(test)]`
   qui **redéfinit à l'identique la struct `ExternalEvent` de la frise** et désérialise notre
   export dedans, en asserant les valeurs. Reprendre aussi comme fixtures les cas verbatim des
   tests de `frise-chronologique/src/storage/import.rs` :
   `[{"date_label":"08/06 14h32","date_start":"2026-06-08","date_end":"2026-06-08","is_range":false,"time_of_day":"14h32","content":["du texte"]}]`.
   Commentaire en tête du module : *à resynchroniser si `import.rs` change*.
8. **Installation** — `completions`, `install.sh`, README (usage, exemples, les deux pièges de
   l'import frise, procédure WSL2).
9. **Déploiement WSL2** — pousser le dépôt, cloner et `cargo install --path .` sur le PC
   secondaire, vérifier `note take` et `note export`.

---

## Vérification de bout en bout

```bash
cargo test                       # dont le test de compatibilité du format frise
cargo clippy -- -D warnings

# parcours réel
export NOTE_DIR=/tmp/note-test
note take "Réunion budget : arbitrage repoussé à octobre" -c Management -t budget,arbitrage
note take "Le client veut un export CSV, pas Akeneo" -c Projets -t export-csv
note list
note tags
note export --format frise --out /tmp/frise-import.json
jq '.[0]' /tmp/frise-import.json      # vérifier date_start "YYYY-MM-DD" et content en tableau
```

Validation finale contre la vraie frise :

1. Sauvegarder `~/.local/share/frise-chronologique/data.json` (l'import est destructif en cas de
   malformation, cf. plus haut).
2. `cd ~/projects/Rust/frise-chronologique && cargo run` → `Fichier > Importer JSON…` →
   `/tmp/frise-import.json`.
3. Vérifier : les événements apparaissent sur la bonne date, sur la ligne de la bonne catégorie,
   titre dérivé correct, tags visibles dans l'infobulle, rapport d'import sans ligne « ignorée ».
