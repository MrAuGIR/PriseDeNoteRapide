use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::filter::FilterOpts;

#[derive(Debug, Parser)]
#[command(
    name = "note",
    version,
    about = "Prise de note rapide en ligne de commande",
    long_about = "Capture une information à la volée, horodatée, avec tags et catégorie.\n\
                  Les notes sont exportables vers frise-chronologique (note export --format frise)."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Prend une note horodatée
    #[command(visible_aliases = ["add", "t"])]
    Take(TakeArgs),

    /// Liste les notes, les plus récentes en dernier
    #[command(visible_alias = "ls")]
    List(ListArgs),

    /// Recherche dans le texte, les titres et les tags
    Search(SearchArgs),

    /// Affiche une note en entier
    Show { id: String },

    /// Modifie une note existante
    Edit(EditArgs),

    /// Supprime une note
    #[command(visible_alias = "remove")]
    Rm {
        id: String,
        /// Ne pas demander confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Liste les tags utilisés, avec leur nombre d'occurrences
    Tags,

    /// Liste les catégories utilisées, avec leur nombre d'occurrences
    #[command(visible_alias = "categories")]
    Cats,

    /// Exporte les notes (format frise par défaut)
    Export(ExportArgs),

    /// Affiche le chemin du fichier de notes
    Path,

    /// Génère le script de complétion pour un shell
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

#[derive(Debug, Args)]
pub struct TakeArgs {
    /// Le texte de la note (les mots sont joints par des espaces).
    /// Pour un texte commençant par un tiret, le faire précéder de `--`.
    #[arg(value_name = "TEXTE")]
    pub text: Vec<String>,

    /// Tag(s) : répétable et acceptant une liste séparée par des virgules (-t a,b -t c)
    #[arg(short = 't', long = "tag", value_name = "TAGS")]
    pub tags: Vec<String>,

    /// Catégorie de la note (détermine la ligne sur laquelle elle apparaît dans la frise)
    #[arg(short = 'c', long = "cat", value_name = "CATÉGORIE")]
    pub category: Option<String>,

    /// Titre explicite (sinon dérivé de la première ligne)
    #[arg(long)]
    pub title: Option<String>,

    /// Antidater : `YYYY-MM-DD` ou `YYYY-MM-DDTHH:MM`
    #[arg(long, value_name = "QUAND")]
    pub at: Option<String>,

    /// Lire le corps de la note sur l'entrée standard
    #[arg(long)]
    pub stdin: bool,
}

#[derive(Debug, Args)]
pub struct ListArgs {
    #[command(flatten)]
    pub filter: FilterOpts,

    /// Afficher le texte complet plutôt qu'un aperçu d'une ligne
    #[arg(short, long)]
    pub full: bool,

    /// Sortie JSON (format interne), pour scripter
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct SearchArgs {
    /// Motif recherché, insensible à la casse
    pub pattern: String,

    #[command(flatten)]
    pub filter: FilterOpts,

    /// Afficher le texte complet
    #[arg(short, long)]
    pub full: bool,

    /// Sortie JSON (format interne)
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct EditArgs {
    /// Préfixe d'identifiant de la note
    pub id: String,

    /// Nouveau texte (sinon ouvre $EDITOR)
    #[arg(long)]
    pub text: Option<String>,

    /// Remplace la catégorie
    #[arg(short = 'c', long = "cat", value_name = "CATÉGORIE")]
    pub category: Option<String>,

    /// Remplace le titre
    #[arg(long)]
    pub title: Option<String>,

    /// Remplace entièrement les tags
    #[arg(short = 't', long = "tag", value_name = "TAGS")]
    pub tags: Vec<String>,

    /// Ajoute des tags aux tags existants
    #[arg(long = "add-tag", value_name = "TAGS")]
    pub add_tags: Vec<String>,
}

#[derive(Debug, Clone, Copy, ValueEnum, PartialEq)]
pub enum ExportFormat {
    /// Tableau JSON lisible par « Fichier > Importer JSON… » de frise-chronologique
    Frise,
    /// Format interne brut (tableau de notes)
    Json,
    /// Markdown groupé par jour
    Md,
}

#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Format de sortie
    #[arg(long, value_enum, default_value = "frise")]
    pub format: ExportFormat,

    /// Fichier de sortie (sinon la sortie standard)
    #[arg(short, long, value_name = "FICHIER")]
    pub out: Option<std::path::PathBuf>,

    /// Format frise : fusionner les notes d'un même jour et d'une même catégorie en un événement
    #[arg(long)]
    pub group_by_day: bool,

    #[command(flatten)]
    pub filter: FilterOpts,
}
