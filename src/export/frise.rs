//! Export vers le format d'import de `frise-chronologique`.
//!
//! Le format cible est un **tableau JSON nu** désérialisé par `ExternalEvent` dans
//! `frise-chronologique/src/storage/import.rs`. Contraintes vérifiées à la source :
//!
//! - `date_start` / `date_end` sont des `chrono::NaiveDate` → `"YYYY-MM-DD"` strict.
//!   Un horodatage RFC3339 complet ferait échouer l'import.
//! - `content` est un tableau de chaînes (rejointes par `\n` côté frise).
//! - `title` absent → la frise dérive le titre depuis `date_label` + première ligne.
//! - `category` absente → la frise applique sa valeur par défaut `"projet"`.
//! - L'axe vertical de la frise est bâti sur `category` seule ; `tags` n'apparaît qu'en
//!   infobulle et dans l'export Markdown.

use std::collections::BTreeSet;

use chrono::NaiveDate;
use serde::Serialize;

use crate::model::Note;

/// Un événement au format attendu par l'import de la frise.
///
/// Les `Option` absentes sont omises plutôt que sérialisées à `null` : les champs de la frise
/// sont tous `#[serde(default)]`, l'omission déclenche donc ses valeurs par défaut.
#[derive(Debug, Serialize, PartialEq)]
pub struct FriseEvent {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    pub date_label: String,
    pub date_start: NaiveDate,
    pub date_end: NaiveDate,
    pub is_range: bool,
    pub time_of_day: Option<String>,
    pub content: Vec<String>,
}

/// Une note → un événement daté à la minute.
pub fn note_to_event(note: &Note) -> FriseEvent {
    let dt = note.created_at;
    FriseEvent {
        title: note.title.clone(),
        category: note.category.clone(),
        tags: if note.tags.is_empty() {
            None
        } else {
            Some(note.tags.clone())
        },
        date_label: dt.format("%d/%m %Hh%M").to_string(),
        date_start: dt.date_naive(),
        date_end: dt.date_naive(),
        is_range: false,
        time_of_day: Some(dt.format("%Hh%M").to_string()),
        content: note.content_lines(),
    }
}

/// Regroupe les notes par (jour, catégorie) en un seul événement par groupe.
///
/// Donne la même forme que le `notes.json` déjà présent dans le dépôt de la frise : une
/// frise moins dense, une entrée par journée et par catégorie.
pub fn group_by_day(notes: &[Note]) -> Vec<FriseEvent> {
    // BTreeMap pour un ordre de sortie déterministe (date puis catégorie).
    let mut groupes: std::collections::BTreeMap<(NaiveDate, String), Vec<&Note>> =
        Default::default();
    for n in notes {
        let cle = (
            n.created_at.date_naive(),
            n.category.clone().unwrap_or_default(),
        );
        groupes.entry(cle).or_default().push(n);
    }

    groupes
        .into_iter()
        .map(|((date, cat), notes)| {
            let mut content = Vec::new();
            let mut tags: BTreeSet<String> = BTreeSet::new();
            for n in &notes {
                content.extend(n.content_lines());
                tags.extend(n.tags.iter().cloned());
            }
            FriseEvent {
                title: None,
                category: if cat.is_empty() { None } else { Some(cat) },
                tags: if tags.is_empty() {
                    None
                } else {
                    Some(tags.into_iter().collect())
                },
                date_label: date.format("%d/%m").to_string(),
                date_start: date,
                date_end: date,
                is_range: false,
                time_of_day: None,
                content,
            }
        })
        .collect()
}

/// Rend le tableau JSON complet, prêt pour `Fichier > Importer JSON…` dans la frise.
pub fn render(notes: &[Note], grouper_par_jour: bool) -> anyhow::Result<String> {
    let events: Vec<FriseEvent> = if grouper_par_jour {
        group_by_day(notes)
    } else {
        notes.iter().map(note_to_event).collect()
    };
    Ok(serde_json::to_string_pretty(&events)?)
}

#[cfg(test)]
mod tests {
    //! Ces tests verrouillent la compatibilité avec la frise.
    //!
    //! `ExternalEvent` ci-dessous est une **copie conforme** de la struct de
    //! `frise-chronologique/src/storage/import.rs`. Si ce fichier change là-bas, la copie
    //! doit être resynchronisée ici — c'est le seul point de couplage entre les deux outils.

    use super::*;
    use crate::model::Note;
    use chrono::{Local, TimeZone};
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    #[allow(dead_code)]
    struct ExternalEvent {
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        category: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        date_label: Option<String>,
        date_start: NaiveDate,
        #[serde(default)]
        date_end: Option<NaiveDate>,
        #[serde(default)]
        is_range: bool,
        #[serde(default)]
        time_of_day: Option<String>,
        #[serde(default)]
        content: Vec<String>,
    }

    fn note(date_heure: &str, texte: &str, cat: Option<&str>, tags: &[&str]) -> Note {
        let ndt = chrono::NaiveDateTime::parse_from_str(date_heure, "%Y-%m-%d %H:%M").unwrap();
        let dt = Local.from_local_datetime(&ndt).unwrap().fixed_offset();
        Note::new(
            dt,
            texte.into(),
            None,
            cat.map(String::from),
            tags.iter().map(|t| t.to_string()).collect(),
        )
    }

    /// Le contrat essentiel : ce que nous produisons, la frise sait le relire.
    #[test]
    fn notre_export_est_desserialisable_par_la_frise() {
        let notes = vec![
            note("2026-06-08 14:32", "du texte", Some("Projets"), &["a", "b"]),
            note("2026-06-09 09:05", "ligne 1\nligne 2", None, &[]),
        ];
        let json = render(&notes, false).unwrap();
        let events: Vec<ExternalEvent> = serde_json::from_str(&json).unwrap();

        assert_eq!(events.len(), 2);
        assert_eq!(
            events[0].date_start,
            NaiveDate::from_ymd_opt(2026, 6, 8).unwrap()
        );
        assert_eq!(events[0].date_end, events[0].date_start.into());
        assert_eq!(events[0].date_label.as_deref(), Some("08/06 14h32"));
        assert_eq!(events[0].time_of_day.as_deref(), Some("14h32"));
        assert_eq!(events[0].category.as_deref(), Some("Projets"));
        assert_eq!(
            events[0].tags.as_deref(),
            Some(&["a".to_string(), "b".to_string()][..])
        );
        assert_eq!(events[0].content, vec!["du texte"]);
        assert!(!events[0].is_range);

        // Sans catégorie ni tags : les clés sont omises, la frise appliquera ses défauts.
        assert!(events[1].category.is_none());
        assert!(events[1].tags.is_none());
        assert_eq!(events[1].content, vec!["ligne 1", "ligne 2"]);
    }

    /// La date doit rester `YYYY-MM-DD` : un RFC3339 casserait l'import côté frise.
    #[test]
    fn les_dates_sont_des_naivedate_pas_des_horodatages() {
        let notes = vec![note("2026-06-08 14:32", "x", None, &[])];
        let json = render(&notes, false).unwrap();
        assert!(json.contains("\"date_start\": \"2026-06-08\""));
        assert!(!json.contains("2026-06-08T"));
    }

    /// Reprise verbatim d'une fixture des tests de `import.rs` : notre struct doit accepter
    /// exactement la même forme, dans les deux sens.
    #[test]
    fn fixture_verbatim_de_la_frise() {
        let raw = r#"[{"date_label":"08/06 14h32","date_start":"2026-06-08","date_end":"2026-06-08","is_range":false,"time_of_day":"14h32","content":["du texte"]}]"#;
        let attendu: Vec<ExternalEvent> = serde_json::from_str(raw).unwrap();
        assert_eq!(attendu.len(), 1);

        let notes = vec![note("2026-06-08 14:32", "du texte", None, &[])];
        let notre: Vec<ExternalEvent> =
            serde_json::from_str(&render(&notes, false).unwrap()).unwrap();
        assert_eq!(notre[0].date_label, attendu[0].date_label);
        assert_eq!(notre[0].date_start, attendu[0].date_start);
        assert_eq!(notre[0].time_of_day, attendu[0].time_of_day);
        assert_eq!(notre[0].content, attendu[0].content);
    }

    #[test]
    fn group_by_day_fusionne_par_jour_et_categorie() {
        let notes = vec![
            note("2026-06-08 09:00", "a", Some("Projets"), &["x"]),
            note("2026-06-08 14:00", "b", Some("Projets"), &["y"]),
            note("2026-06-08 15:00", "c", Some("RH"), &[]),
            note("2026-06-09 10:00", "d", Some("Projets"), &[]),
        ];
        let events = group_by_day(&notes);
        assert_eq!(events.len(), 3);

        let projets_8 = &events[0];
        assert_eq!(projets_8.category.as_deref(), Some("Projets"));
        assert_eq!(projets_8.content, vec!["a", "b"]);
        assert_eq!(projets_8.tags, Some(vec!["x".to_string(), "y".to_string()]));
        assert_eq!(projets_8.date_label, "08/06");
        assert!(projets_8.time_of_day.is_none());

        // Reste lisible par la frise.
        let json = render(&notes, true).unwrap();
        let _: Vec<ExternalEvent> = serde_json::from_str(&json).unwrap();
    }
}
