use crate::model::Note;

/// Rend les notes en Markdown, groupées par jour, dans l'ordre chronologique.
pub fn render(notes: &[Note]) -> String {
    let mut out = String::from("# Notes\n");
    let mut jour_courant = None;
    for n in notes {
        let jour = n.created_at.date_naive();
        if jour_courant != Some(jour) {
            out.push_str(&format!("\n## {}\n\n", jour.format("%Y-%m-%d")));
            jour_courant = Some(jour);
        }
        out.push_str(&format!(
            "### {} — {}\n\n",
            n.created_at.format("%Hh%M"),
            n.summary()
        ));
        let mut meta = Vec::new();
        if let Some(c) = &n.category {
            meta.push(format!("catégorie : {c}"));
        }
        if !n.tags.is_empty() {
            meta.push(format!("tags : {}", n.tags.join(", ")));
        }
        if !meta.is_empty() {
            out.push_str(&format!("*{}*\n\n", meta.join(" · ")));
        }
        for ligne in n.content_lines() {
            out.push_str(&ligne);
            out.push('\n');
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Local, NaiveDateTime, TimeZone};

    fn note(date_heure: &str, texte: &str, cat: Option<&str>, tags: &[&str]) -> Note {
        let ndt = NaiveDateTime::parse_from_str(date_heure, "%Y-%m-%d %H:%M").unwrap();
        let dt = Local.from_local_datetime(&ndt).unwrap().fixed_offset();
        Note::new(
            dt,
            texte.into(),
            None,
            cat.map(String::from),
            tags.iter().map(|t| t.to_string()).collect(),
        )
    }

    #[test]
    fn aucune_note_donne_juste_le_titre() {
        assert_eq!(render(&[]), "# Notes\n");
    }

    #[test]
    fn notes_groupees_par_jour() {
        let notes = vec![
            note("2026-06-08 09:00", "a", None, &[]),
            note("2026-06-08 14:30", "b", None, &[]),
            note("2026-06-09 10:00", "c", None, &[]),
        ];
        let md = render(&notes);
        assert_eq!(md.matches("\n## ").count(), 2);
        assert!(md.contains("## 2026-06-08"));
        assert!(md.contains("## 2026-06-09"));
        assert!(md.contains("### 09h00 — a"));
        assert!(md.contains("### 14h30 — b"));
        assert!(md.find("2026-06-08").unwrap() < md.find("2026-06-09").unwrap());
    }

    #[test]
    fn metadonnees_categorie_et_tags() {
        let notes = vec![note("2026-06-08 09:00", "x", Some("Projets"), &["a", "b"])];
        let md = render(&notes);
        assert!(md.contains("*catégorie : Projets · tags : a, b*"));

        let notes = vec![note("2026-06-08 09:00", "x", None, &["a"])];
        assert!(render(&notes).contains("*tags : a*"));
    }

    #[test]
    fn sans_metadonnees_pas_de_ligne_en_italique() {
        let notes = vec![note("2026-06-08 09:00", "ligne 1\nligne 2", None, &[])];
        let md = render(&notes);
        assert!(!md.contains('*'));
        assert!(md.contains("ligne 1\nligne 2\n"));
    }
}
