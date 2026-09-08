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
