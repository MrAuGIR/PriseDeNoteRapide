use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Une note telle qu'elle est stockée sur disque : une par ligne du fichier JSONL.
///
/// L'horodatage garde l'heure complète et le décalage horaire. C'est l'export vers la frise
/// qui dégrade vers la date nue (`NaiveDate`) qu'elle sait lire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub created_at: DateTime<FixedOffset>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

impl Note {
    pub fn new(
        created_at: DateTime<FixedOffset>,
        text: String,
        title: Option<String>,
        category: Option<String>,
        tags: Vec<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            created_at,
            text,
            title,
            category,
            tags,
        }
    }

    /// Identifiant court affiché dans les listes, à la manière de git.
    pub fn short_id(&self) -> String {
        self.id.simple().to_string()[..8].to_string()
    }

    /// Première ligne non vide, pour l'aperçu en liste.
    pub fn summary(&self) -> &str {
        if let Some(t) = &self.title {
            return t;
        }
        self.text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("")
    }

    /// Le texte découpé en lignes, forme attendue par le champ `content` de la frise.
    pub fn content_lines(&self) -> Vec<String> {
        let lines: Vec<String> = self
            .text
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect();
        // On enlève les lignes vides en tête et en queue, on garde celles du milieu.
        let start = lines.iter().position(|l| !l.trim().is_empty());
        let end = lines.iter().rposition(|l| !l.trim().is_empty());
        match (start, end) {
            (Some(s), Some(e)) => lines[s..=e].to_vec(),
            _ => Vec::new(),
        }
    }
}

/// Normalise un tag : minuscules, espaces et underscores en tirets, accents conservés.
///
/// Cohérent avec la convention déjà en place dans les données de la frise
/// (`épi-saveur`, `conflit-d-intérêt`).
pub fn normalize_tag(raw: &str) -> Option<String> {
    let t: String = raw
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_whitespace() || c == '_' {
                '-'
            } else {
                c
            }
        })
        .collect();
    let t = t.trim_matches('-').to_string();
    if t.is_empty() {
        None
    } else {
        Some(t)
    }
}

/// Aplatit des `--tag a,b --tag c` en une liste normalisée sans doublons, ordre préservé.
pub fn normalize_tags<I: IntoIterator<Item = S>, S: AsRef<str>>(raw: I) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for chunk in raw {
        for part in chunk.as_ref().split(',') {
            if let Some(tag) = normalize_tag(part) {
                if !out.contains(&tag) {
                    out.push(tag);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tags_normalises_et_dedoublonnes() {
        let tags = normalize_tags(["Budget, arbitrage", "budget", "Épi Saveur"]);
        assert_eq!(tags, vec!["budget", "arbitrage", "épi-saveur"]);
    }

    #[test]
    fn tags_vides_ignores() {
        assert_eq!(normalize_tags([" , ,", "-"]), Vec::<String>::new());
    }

    #[test]
    fn content_lines_rogne_les_bords_pas_le_milieu() {
        let n = Note::new(
            chrono::Local::now().fixed_offset(),
            "\na\n\nb\n\n".to_string(),
            None,
            None,
            vec![],
        );
        assert_eq!(n.content_lines(), vec!["a", "", "b"]);
    }

    #[test]
    fn summary_prend_le_titre_sinon_la_premiere_ligne() {
        let now = chrono::Local::now().fixed_offset();
        let n = Note::new(now, "  \nligne utile".into(), None, None, vec![]);
        assert_eq!(n.summary(), "ligne utile");
        let n = Note::new(
            now,
            "ligne utile".into(),
            Some("Titre".into()),
            None,
            vec![],
        );
        assert_eq!(n.summary(), "Titre");
    }
}
