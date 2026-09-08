use anyhow::{bail, Result};
use chrono::{Duration, Local, NaiveDate};
use clap::Args;

use crate::model::{normalize_tag, Note};

/// Filtres partagés par `list`, `search` et `export`.
#[derive(Debug, Args, Clone, Default)]
pub struct FilterOpts {
    /// Notes à partir de cette date : `YYYY-MM-DD`, ou relatif (`today`, `yesterday`, `7d`, `2w`, `3m`)
    #[arg(long, value_name = "DATE")]
    pub since: Option<String>,

    /// Notes jusqu'à cette date incluse (mêmes formats que --since)
    #[arg(long, value_name = "DATE")]
    pub until: Option<String>,

    /// Ne garder que les notes portant ce tag (répétable, cumulatif : toutes doivent être présentes)
    #[arg(short = 't', long = "tag", value_name = "TAG")]
    pub tags: Vec<String>,

    /// Ne garder que les notes de cette catégorie (insensible à la casse)
    #[arg(short = 'c', long = "cat", value_name = "CATÉGORIE")]
    pub category: Option<String>,

    /// Nombre maximum de notes (les plus récentes)
    #[arg(short = 'n', long, value_name = "N")]
    pub limit: Option<usize>,
}

/// Interprète une date de filtre : absolue `YYYY-MM-DD`, ou relative au jour courant.
pub fn parse_date(s: &str) -> Result<NaiveDate> {
    let s = s.trim();
    let today = Local::now().date_naive();
    match s.to_lowercase().as_str() {
        "today" | "aujourdhui" | "aujourd'hui" => return Ok(today),
        "yesterday" | "hier" => return Ok(today - Duration::days(1)),
        _ => {}
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(d);
    }
    // Formes relatives : 7d, 2w, 3m, 1y
    let (nombre, unite) = s.split_at(s.len().saturating_sub(1));
    if let Ok(n) = nombre.parse::<i64>() {
        let jours = match unite {
            "d" | "j" => n,
            "w" | "s" => n * 7,
            "m" => n * 30,
            "y" | "a" => n * 365,
            _ => bail!("unité inconnue dans « {s} » (attendu d, w, m ou y)"),
        };
        return Ok(today - Duration::days(jours));
    }
    bail!("date « {s} » non comprise (attendu YYYY-MM-DD, today, yesterday, 7d, 2w, 3m, 1y)")
}

impl FilterOpts {
    /// Applique les filtres et trie chronologiquement (les notes antidatées avec `--at` sont
    /// donc remises à leur place) ; `--limit` garde les N plus récentes.
    pub fn apply(&self, notes: Vec<Note>) -> Result<Vec<Note>> {
        let since = self.since.as_deref().map(parse_date).transpose()?;
        let until = self.until.as_deref().map(parse_date).transpose()?;
        let tags: Vec<String> = self.tags.iter().filter_map(|t| normalize_tag(t)).collect();
        let cat = self.category.as_ref().map(|c| c.trim().to_lowercase());

        let mut out: Vec<Note> = notes
            .into_iter()
            .filter(|n| {
                let d = n.created_at.date_naive();
                if since.is_some_and(|s| d < s) {
                    return false;
                }
                if until.is_some_and(|u| d > u) {
                    return false;
                }
                if !tags.iter().all(|t| n.tags.contains(t)) {
                    return false;
                }
                if let Some(c) = &cat {
                    match &n.category {
                        Some(nc) if nc.trim().to_lowercase() == *c => {}
                        _ => return false,
                    }
                }
                true
            })
            .collect();

        // Tri stable : `--at` permet d'insérer une note hors de l'ordre d'écriture.
        out.sort_by_key(|n| n.created_at);

        if let Some(limit) = self.limit {
            if out.len() > limit {
                out.drain(..out.len() - limit);
            }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn note_le(date: &str, cat: Option<&str>, tags: &[&str]) -> Note {
        let d = NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
        let dt = Local
            .from_local_datetime(&d.and_hms_opt(10, 0, 0).unwrap())
            .unwrap()
            .fixed_offset();
        Note::new(
            dt,
            "texte".into(),
            None,
            cat.map(String::from),
            tags.iter().map(|t| t.to_string()).collect(),
        )
    }

    #[test]
    fn parse_date_absolue_et_relative() {
        assert_eq!(
            parse_date("2026-06-08").unwrap(),
            NaiveDate::from_ymd_opt(2026, 6, 8).unwrap()
        );
        assert_eq!(parse_date("today").unwrap(), Local::now().date_naive());
        assert_eq!(
            parse_date("7d").unwrap(),
            Local::now().date_naive() - Duration::days(7)
        );
        assert!(parse_date("n'importe quoi").is_err());
    }

    #[test]
    fn filtre_par_date_tag_et_categorie() {
        let notes = vec![
            note_le("2026-01-01", Some("Projets"), &["a"]),
            note_le("2026-02-01", Some("RH"), &["a", "b"]),
            note_le("2026-03-01", Some("Projets"), &["b"]),
        ];

        let f = FilterOpts {
            since: Some("2026-02-01".into()),
            ..Default::default()
        };
        assert_eq!(f.apply(notes.clone()).unwrap().len(), 2);

        let f = FilterOpts {
            until: Some("2026-02-01".into()),
            ..Default::default()
        };
        assert_eq!(f.apply(notes.clone()).unwrap().len(), 2);

        let f = FilterOpts {
            tags: vec!["A".into()],
            ..Default::default()
        };
        assert_eq!(f.apply(notes.clone()).unwrap().len(), 2);

        let f = FilterOpts {
            tags: vec!["a".into(), "b".into()],
            ..Default::default()
        };
        assert_eq!(f.apply(notes.clone()).unwrap().len(), 1);

        let f = FilterOpts {
            category: Some("projets".into()),
            ..Default::default()
        };
        assert_eq!(f.apply(notes.clone()).unwrap().len(), 2);
    }

    #[test]
    fn limit_garde_les_plus_recentes() {
        let notes = vec![
            note_le("2026-01-01", None, &[]),
            note_le("2026-02-01", None, &[]),
            note_le("2026-03-01", None, &[]),
        ];
        let f = FilterOpts {
            limit: Some(2),
            ..Default::default()
        };
        let out = f.apply(notes).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].created_at.date_naive().to_string(), "2026-02-01");
    }
}
