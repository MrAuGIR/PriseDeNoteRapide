use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::model::Note;

/// Emplacement du fichier de notes : `$NOTE_DIR/notes.jsonl` si la variable est définie,
/// sinon `~/.local/share/prise-note-rapide/notes.jsonl`.
pub fn data_path() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("NOTE_DIR") {
        if !dir.is_empty() {
            return Ok(PathBuf::from(dir).join("notes.jsonl"));
        }
    }
    let base = dirs::data_local_dir().context(
        "impossible de déterminer le répertoire de données local (XDG_DATA_HOME / HOME)",
    )?;
    Ok(base.join("prise-note-rapide").join("notes.jsonl"))
}

/// Ajoute une note en fin de fichier.
///
/// Une seule écriture en `O_APPEND` : sur Linux une écriture courte est atomique, deux
/// terminaux qui prennent une note en même temps ne peuvent pas s'entremêler.
pub fn append(path: &Path, note: &Note) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("création du répertoire {}", parent.display()))?;
    }
    let mut line = serde_json::to_string(note).context("sérialisation de la note")?;
    line.push('\n');
    let mut f = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("ouverture de {}", path.display()))?;
    f.write_all(line.as_bytes())
        .with_context(|| format!("écriture dans {}", path.display()))?;
    Ok(())
}

/// Lit toutes les notes. Une ligne illisible est signalée sur stderr et ignorée : on ne perd
/// jamais le reste du fichier à cause d'une seule ligne abîmée.
pub fn read_all(path: &Path) -> Result<Vec<Note>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let f = File::open(path).with_context(|| format!("ouverture de {}", path.display()))?;
    let mut notes = Vec::new();
    for (i, line) in BufReader::new(f).lines().enumerate() {
        let line =
            line.with_context(|| format!("lecture de {} ligne {}", path.display(), i + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Note>(&line) {
            Ok(n) => notes.push(n),
            Err(e) => eprintln!(
                "note: ligne {} ignorée dans {} ({e})",
                i + 1,
                path.display()
            ),
        }
    }
    Ok(notes)
}

/// Réécrit intégralement le fichier via un temporaire du même répertoire puis un `rename`
/// atomique : jamais de fichier tronqué si l'écriture est interrompue.
pub fn rewrite(path: &Path, notes: &[Note]) -> Result<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent)
        .with_context(|| format!("création du répertoire {}", parent.display()))?;
    let tmp = parent.join(format!(".notes.jsonl.tmp.{}", std::process::id()));
    {
        let mut f = File::create(&tmp).with_context(|| format!("création de {}", tmp.display()))?;
        for n in notes {
            let mut line = serde_json::to_string(n).context("sérialisation de la note")?;
            line.push('\n');
            f.write_all(line.as_bytes())?;
        }
        f.sync_all().ok();
    }
    fs::rename(&tmp, path).with_context(|| format!("remplacement de {}", path.display()))?;
    Ok(())
}

/// Retrouve l'indice d'une note par préfixe d'identifiant, à la manière de git.
pub fn resolve_id_prefix(notes: &[Note], prefix: &str) -> Result<usize> {
    let needle = prefix.trim().to_lowercase().replace('-', "");
    if needle.is_empty() {
        bail!("identifiant vide");
    }
    let matches: Vec<usize> = notes
        .iter()
        .enumerate()
        .filter(|(_, n)| n.id.simple().to_string().starts_with(&needle))
        .map(|(i, _)| i)
        .collect();
    match matches.len() {
        0 => bail!("aucune note ne correspond à « {prefix} »"),
        1 => Ok(matches[0]),
        n => {
            let ids: Vec<String> = matches.iter().map(|&i| notes[i].short_id()).collect();
            bail!(
                "« {prefix} » est ambigu, {n} notes correspondent : {}",
                ids.join(", ")
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Note;
    use chrono::Local;

    fn tmpdir(nom: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("pnr-test-{}-{}", nom, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn note(txt: &str) -> Note {
        Note::new(Local::now().fixed_offset(), txt.into(), None, None, vec![])
    }

    #[test]
    fn aller_retour_append_read() {
        let d = tmpdir("append");
        let p = d.join("notes.jsonl");
        append(&p, &note("une")).unwrap();
        append(&p, &note("deux")).unwrap();
        let lues = read_all(&p).unwrap();
        assert_eq!(lues.len(), 2);
        assert_eq!(lues[1].text, "deux");
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn ligne_corrompue_ignoree_sans_perdre_les_autres() {
        let d = tmpdir("corrompu");
        let p = d.join("notes.jsonl");
        append(&p, &note("bonne")).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(b"{ceci n'est pas du json}\n")
            .unwrap();
        append(&p, &note("encore bonne")).unwrap();
        let lues = read_all(&p).unwrap();
        assert_eq!(lues.len(), 2);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn fichier_absent_donne_liste_vide() {
        let p = std::env::temp_dir().join("pnr-inexistant-xyz/notes.jsonl");
        assert!(read_all(&p).unwrap().is_empty());
    }

    #[test]
    fn rewrite_remplace_le_contenu() {
        let d = tmpdir("rewrite");
        let p = d.join("notes.jsonl");
        append(&p, &note("a")).unwrap();
        append(&p, &note("b")).unwrap();
        let mut notes = read_all(&p).unwrap();
        notes.remove(0);
        rewrite(&p, &notes).unwrap();
        let lues = read_all(&p).unwrap();
        assert_eq!(lues.len(), 1);
        assert_eq!(lues[0].text, "b");
        fs::remove_dir_all(&d).unwrap();
    }

    /// Un seul test touche `NOTE_DIR` : les tests tournent en parallèle et partagent l'env.
    #[test]
    fn data_path_respecte_note_dir() {
        let avant = std::env::var_os("NOTE_DIR");

        std::env::set_var("NOTE_DIR", "/tmp/pnr-ailleurs");
        assert_eq!(
            data_path().unwrap(),
            PathBuf::from("/tmp/pnr-ailleurs/notes.jsonl")
        );

        // Vide : on retombe sur le répertoire de données local.
        std::env::set_var("NOTE_DIR", "");
        let p = data_path().unwrap();
        assert!(p.ends_with("prise-note-rapide/notes.jsonl"));

        match avant {
            Some(v) => std::env::set_var("NOTE_DIR", v),
            None => std::env::remove_var("NOTE_DIR"),
        }
    }

    #[test]
    fn rewrite_cree_le_repertoire_parent() {
        let d = tmpdir("rewrite-parent");
        let p = d.join("sous").join("notes.jsonl");
        rewrite(&p, &[note("x")]).unwrap();
        assert_eq!(read_all(&p).unwrap().len(), 1);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn lignes_vides_ignorees() {
        let d = tmpdir("vides");
        let p = d.join("notes.jsonl");
        append(&p, &note("a")).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&p)
            .unwrap()
            .write_all(b"\n   \n")
            .unwrap();
        append(&p, &note("b")).unwrap();
        assert_eq!(read_all(&p).unwrap().len(), 2);
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn prefixe_ambigu_signale() {
        let notes = vec![note("a"), note("b")];
        assert!(resolve_id_prefix(&notes, "").is_err());
        assert!(resolve_id_prefix(&notes, "zzzzzzzz").is_err());
        let court = notes[0].short_id();
        assert_eq!(resolve_id_prefix(&notes, &court).unwrap(), 0);
        // Les tirets et la casse de l'UUID complet sont tolérés.
        let complet = notes[1].id.to_string().to_uppercase();
        assert_eq!(resolve_id_prefix(&notes, &complet).unwrap(), 1);
    }

    #[test]
    fn prefixe_partage_par_plusieurs_notes_est_ambigu() {
        let mut a = note("a");
        let mut b = note("b");
        a.id = uuid::Uuid::parse_str("12345678-0000-4000-8000-000000000001").unwrap();
        b.id = uuid::Uuid::parse_str("12345678-0000-4000-8000-000000000002").unwrap();
        let err = resolve_id_prefix(&[a, b], "1234").unwrap_err().to_string();
        assert!(err.contains("ambigu"), "{err}");
    }
}
