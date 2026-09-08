mod cli;
mod export;
mod filter;
mod model;
mod store;

use std::collections::BTreeMap;
use std::io::{IsTerminal, Read, Write};
use std::path::Path;

use anyhow::{bail, Context, Result};
use chrono::{Local, NaiveDate, NaiveDateTime, TimeZone};
use clap::{CommandFactory, Parser};

use cli::{Cli, Command, EditArgs, ExportArgs, ExportFormat, ListArgs, SearchArgs, TakeArgs};
use model::{normalize_tags, Note};

fn main() {
    if let Err(e) = run() {
        eprintln!("note: {e:#}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let cli = Cli::parse();
    let path = store::data_path()?;

    match cli.command {
        Command::Take(args) => cmd_take(&path, args),
        Command::List(args) => cmd_list(&path, args),
        Command::Search(args) => cmd_search(&path, args),
        Command::Show { id } => cmd_show(&path, &id),
        Command::Edit(args) => cmd_edit(&path, args),
        Command::Rm { id, force } => cmd_rm(&path, &id, force),
        Command::Tags => cmd_compter(&path, true),
        Command::Cats => cmd_compter(&path, false),
        Command::Export(args) => cmd_export(&path, args),
        Command::Path => {
            println!("{}", path.display());
            Ok(())
        }
        Command::Completions { shell } => {
            let mut cmd = Cli::command();
            clap_complete::generate(shell, &mut cmd, "note", &mut std::io::stdout());
            Ok(())
        }
    }
}

/// Interprète `--at` : date seule (minuit) ou date + heure.
fn parse_at(s: &str) -> Result<chrono::DateTime<chrono::FixedOffset>> {
    let s = s.trim();
    let naive = if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M") {
        dt
    } else if let Ok(dt) = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M") {
        dt
    } else if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        d.and_hms_opt(0, 0, 0).unwrap()
    } else {
        bail!("date « {s} » non comprise (attendu YYYY-MM-DD ou YYYY-MM-DDTHH:MM)");
    };
    Local
        .from_local_datetime(&naive)
        .earliest()
        .map(|dt| dt.fixed_offset())
        .with_context(|| format!("heure locale ambiguë ou inexistante : {s}"))
}

fn cmd_take(path: &Path, args: TakeArgs) -> Result<()> {
    let mut text = args.text.join(" ").trim().to_string();

    if args.stdin {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .context("lecture de l'entrée standard")?;
        let buf = buf.trim_end();
        if text.is_empty() {
            text = buf.to_string();
        } else if !buf.is_empty() {
            text = format!("{text}\n{buf}");
        }
    }

    if text.trim().is_empty() {
        bail!("note vide — donnez un texte, ou utilisez --stdin");
    }

    let created_at = match &args.at {
        Some(s) => parse_at(s)?,
        None => Local::now().fixed_offset(),
    };

    let note = Note::new(
        created_at,
        text,
        args.title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty()),
        args.category
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty()),
        normalize_tags(args.tags),
    );

    store::append(path, &note)?;
    println!(
        "{}  {}",
        note.short_id(),
        note.created_at.format("%Y-%m-%d %Hh%M")
    );
    Ok(())
}

fn cmd_list(path: &Path, args: ListArgs) -> Result<()> {
    let notes = args.filter.apply(store::read_all(path)?)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&notes)?);
        return Ok(());
    }
    afficher(&notes, args.full);
    Ok(())
}

fn cmd_search(path: &Path, args: SearchArgs) -> Result<()> {
    let motif = args.pattern.to_lowercase();
    let notes: Vec<Note> = args
        .filter
        .apply(store::read_all(path)?)?
        .into_iter()
        .filter(|n| {
            n.text.to_lowercase().contains(&motif)
                || n.title
                    .as_deref()
                    .is_some_and(|t| t.to_lowercase().contains(&motif))
                || n.tags.iter().any(|t| t.contains(&motif))
                || n.category
                    .as_deref()
                    .is_some_and(|c| c.to_lowercase().contains(&motif))
        })
        .collect();

    if args.json {
        println!("{}", serde_json::to_string_pretty(&notes)?);
        return Ok(());
    }
    afficher(&notes, args.full);
    Ok(())
}

fn afficher(notes: &[Note], complet: bool) {
    if notes.is_empty() {
        eprintln!("aucune note");
        return;
    }
    for n in notes {
        let mut ligne = format!(
            "{}  {}",
            n.short_id(),
            n.created_at.format("%Y-%m-%d %Hh%M")
        );
        if let Some(c) = &n.category {
            ligne.push_str(&format!("  [{c}]"));
        }
        if !n.tags.is_empty() {
            ligne.push_str(&format!("  #{}", n.tags.join(" #")));
        }
        println!("{ligne}");
        if complet {
            for l in n.content_lines() {
                println!("    {l}");
            }
            println!();
        } else {
            println!("    {}", n.summary());
        }
    }
}

fn cmd_show(path: &Path, id: &str) -> Result<()> {
    let notes = store::read_all(path)?;
    let i = store::resolve_id_prefix(&notes, id)?;
    let n = &notes[i];
    println!("id         {}", n.id);
    println!("date       {}", n.created_at.format("%Y-%m-%d %Hh%M %z"));
    if let Some(t) = &n.title {
        println!("titre      {t}");
    }
    println!("catégorie  {}", n.category.as_deref().unwrap_or("—"));
    println!(
        "tags       {}",
        if n.tags.is_empty() {
            "—".to_string()
        } else {
            n.tags.join(", ")
        }
    );
    println!();
    println!("{}", n.text);
    Ok(())
}

fn cmd_edit(path: &Path, args: EditArgs) -> Result<()> {
    let mut notes = store::read_all(path)?;
    let i = store::resolve_id_prefix(&notes, &args.id)?;

    let modifie_metadonnees = args.category.is_some()
        || args.title.is_some()
        || !args.tags.is_empty()
        || !args.add_tags.is_empty();

    if let Some(t) = args.text {
        notes[i].text = t;
    } else if !modifie_metadonnees {
        let nouveau = editer_dans_editeur(&notes[i].text)?;
        if nouveau.trim().is_empty() {
            bail!("texte vide, modification annulée");
        }
        notes[i].text = nouveau.trim_end().to_string();
    }

    if let Some(c) = args.category {
        let c = c.trim().to_string();
        notes[i].category = if c.is_empty() { None } else { Some(c) };
    }
    if let Some(t) = args.title {
        let t = t.trim().to_string();
        notes[i].title = if t.is_empty() { None } else { Some(t) };
    }
    if !args.tags.is_empty() {
        notes[i].tags = normalize_tags(args.tags);
    }
    if !args.add_tags.is_empty() {
        for t in normalize_tags(args.add_tags) {
            if !notes[i].tags.contains(&t) {
                notes[i].tags.push(t);
            }
        }
    }

    let court = notes[i].short_id();
    store::rewrite(path, &notes)?;
    println!("{court}  modifiée");
    Ok(())
}

/// Ouvre `$EDITOR` sur une copie temporaire du texte et renvoie le contenu enregistré.
fn editer_dans_editeur(initial: &str) -> Result<String> {
    let editeur = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".to_string());
    let tmp = std::env::temp_dir().join(format!("note-edit-{}.md", std::process::id()));
    std::fs::write(&tmp, initial).with_context(|| format!("écriture de {}", tmp.display()))?;

    let statut = std::process::Command::new(&editeur)
        .arg(&tmp)
        .status()
        .with_context(|| format!("lancement de l'éditeur « {editeur} »"))?;
    if !statut.success() {
        let _ = std::fs::remove_file(&tmp);
        bail!("l'éditeur « {editeur} » s'est terminé en erreur");
    }

    let contenu = std::fs::read_to_string(&tmp)?;
    let _ = std::fs::remove_file(&tmp);
    Ok(contenu)
}

fn cmd_rm(path: &Path, id: &str, force: bool) -> Result<()> {
    let mut notes = store::read_all(path)?;
    let i = store::resolve_id_prefix(&notes, id)?;
    let court = notes[i].short_id();

    if !force {
        if !std::io::stdin().is_terminal() {
            bail!("suppression non confirmée (entrée non interactive) — utilisez --force");
        }
        print!("Supprimer « {} » ? [o/N] ", notes[i].summary());
        std::io::stdout().flush()?;
        let mut rep = String::new();
        std::io::stdin().read_line(&mut rep)?;
        if !matches!(
            rep.trim().to_lowercase().as_str(),
            "o" | "oui" | "y" | "yes"
        ) {
            println!("annulé");
            return Ok(());
        }
    }

    notes.remove(i);
    store::rewrite(path, &notes)?;
    println!("{court}  supprimée");
    Ok(())
}

fn cmd_compter(path: &Path, tags: bool) -> Result<()> {
    let notes = store::read_all(path)?;
    let mut compte: BTreeMap<String, usize> = BTreeMap::new();
    for n in &notes {
        if tags {
            for t in &n.tags {
                *compte.entry(t.clone()).or_default() += 1;
            }
        } else if let Some(c) = &n.category {
            *compte.entry(c.clone()).or_default() += 1;
        }
    }
    if compte.is_empty() {
        eprintln!(
            "aucun{} enregistré",
            if tags { " tag" } else { "e catégorie" }
        );
        return Ok(());
    }
    // Les plus fréquents d'abord, puis ordre alphabétique.
    let mut lignes: Vec<(String, usize)> = compte.into_iter().collect();
    lignes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let largeur = lignes
        .iter()
        .map(|(k, _)| k.chars().count())
        .max()
        .unwrap_or(0);
    for (k, v) in lignes {
        println!("{k:largeur$}  {v}");
    }
    Ok(())
}

fn cmd_export(path: &Path, args: ExportArgs) -> Result<()> {
    let notes = args.filter.apply(store::read_all(path)?)?;

    let rendu = match args.format {
        ExportFormat::Frise => export::frise::render(&notes, args.group_by_day)?,
        ExportFormat::Json => serde_json::to_string_pretty(&notes)?,
        ExportFormat::Md => export::markdown::render(&notes),
    };

    match &args.out {
        Some(p) => {
            if let Some(parent) = p.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
            std::fs::write(p, &rendu).with_context(|| format!("écriture de {}", p.display()))?;
            eprintln!("{} note(s) écrite(s) dans {}", notes.len(), p.display());
        }
        None => print!("{rendu}"),
    }
    Ok(())
}
