//! Tests de bout en bout du binaire `note`.
//!
//! Chaque test travaille dans un répertoire temporaire désigné par `NOTE_DIR` : aucun
//! risque de toucher aux vraies notes de l'utilisateur.

use std::path::{Path, PathBuf};

use assert_cmd::Command;
use predicates::prelude::*;
use predicates::str::contains;
use serde_json::Value;
use tempfile::TempDir;

struct Env {
    dir: TempDir,
}

impl Env {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
        }
    }

    fn note(&self) -> Command {
        let mut c = Command::cargo_bin("note").unwrap();
        c.env("NOTE_DIR", self.dir.path())
            .env_remove("VISUAL")
            .env_remove("EDITOR");
        c
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Prend une note et renvoie son identifiant court.
    fn take(&self, args: &[&str]) -> String {
        let out = self.note().arg("take").args(args).assert().success();
        let stdout = String::from_utf8(out.get_output().stdout.clone()).unwrap();
        stdout.split_whitespace().next().unwrap().to_string()
    }

    fn json(&self, args: &[&str]) -> Vec<Value> {
        let out = self.note().args(args).assert().success();
        let v: Value = serde_json::from_slice(&out.get_output().stdout).unwrap();
        v.as_array().unwrap().clone()
    }

    fn all(&self) -> Vec<Value> {
        self.json(&["list", "--json"])
    }

    /// Script shell servant d'éditeur pour `note edit` sans `--text`.
    fn editeur(&self, corps: &str) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let p = self.path().join("editeur.sh");
        std::fs::write(&p, format!("#!/bin/sh\n{corps}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }
}

// ---------------------------------------------------------------- take

#[test]
fn take_affiche_id_court_et_date() {
    let env = Env::new();
    env.note()
        .args(["take", "bonjour", "le", "monde"])
        .assert()
        .success()
        .stdout(
            predicate::str::is_match(r"^[0-9a-f]{8}  \d{4}-\d{2}-\d{2} \d{2}h\d{2}\n$").unwrap(),
        );
    let notes = env.all();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["text"], "bonjour le monde");
}

#[test]
fn take_avec_metadonnees() {
    let env = Env::new();
    env.take(&[
        "texte",
        "-c",
        " Projets ",
        "-t",
        "Budget, arbitrage",
        "-t",
        "budget",
        "--title",
        "Mon titre",
    ]);
    let n = &env.all()[0];
    assert_eq!(n["category"], "Projets");
    assert_eq!(n["title"], "Mon titre");
    assert_eq!(n["tags"], serde_json::json!(["budget", "arbitrage"]));
}

#[test]
fn take_titre_et_categorie_vides_ignores() {
    let env = Env::new();
    env.take(&["texte", "-c", "  ", "--title", ""]);
    let n = &env.all()[0];
    assert!(n.get("category").is_none());
    assert!(n.get("title").is_none());
}

#[test]
fn take_antidate() {
    let env = Env::new();
    env.note()
        .args(["take", "x", "--at", "2026-01-02"])
        .assert()
        .success()
        .stdout(contains("2026-01-02 00h00"));
    env.note()
        .args(["take", "y", "--at", "2026-01-02T10:30"])
        .assert()
        .success()
        .stdout(contains("2026-01-02 10h30"));
}

#[test]
fn take_date_invalide() {
    let env = Env::new();
    env.note()
        .args(["take", "x", "--at", "demain"])
        .assert()
        .failure()
        .stderr(contains("non comprise"));
}

#[test]
fn take_note_vide_refusee() {
    let env = Env::new();
    env.note()
        .args(["take", "   "])
        .assert()
        .code(1)
        .stderr(contains("note vide"));
    env.note().arg("take").assert().code(1);
}

#[test]
fn take_depuis_stdin() {
    let env = Env::new();
    env.note()
        .args(["take", "--stdin"])
        .write_stdin("corps\nsur deux lignes\n")
        .assert()
        .success();
    env.note()
        .args(["take", "entête", "--stdin"])
        .write_stdin("suite\n")
        .assert()
        .success();
    env.note()
        .args(["take", "seul", "--stdin"])
        .write_stdin("")
        .assert()
        .success();
    let notes = env.all();
    assert_eq!(notes[0]["text"], "corps\nsur deux lignes");
    assert_eq!(notes[1]["text"], "entête\nsuite");
    assert_eq!(notes[2]["text"], "seul");
}

#[test]
fn take_stdin_vide_refuse() {
    let env = Env::new();
    env.note()
        .args(["take", "--stdin"])
        .write_stdin("\n\n")
        .assert()
        .failure();
}

// ---------------------------------------------------------------- list

#[test]
fn list_vide() {
    let env = Env::new();
    env.note()
        .arg("list")
        .assert()
        .success()
        .stdout("")
        .stderr(contains("aucune note"));
}

#[test]
fn list_apercu_et_complet() {
    let env = Env::new();
    env.take(&["première ligne\ndeuxième ligne", "-c", "RH", "-t", "a,b"]);
    env.note()
        .arg("ls")
        .assert()
        .success()
        .stdout(contains("[RH]"))
        .stdout(contains("#a #b"))
        .stdout(contains("    première ligne"))
        .stdout(contains("deuxième").not());
    env.note()
        .args(["list", "--full"])
        .assert()
        .success()
        .stdout(contains("    deuxième ligne"));
}

#[test]
fn list_filtres() {
    let env = Env::new();
    env.take(&["a", "--at", "2026-01-01", "-c", "Projets", "-t", "x"]);
    env.take(&["b", "--at", "2026-02-01", "-c", "RH", "-t", "x,y"]);
    env.take(&["c", "--at", "2026-03-01", "-c", "projets"]);

    let textes = |args: &[&str]| -> Vec<String> {
        let mut a = vec!["list", "--json"];
        a.extend_from_slice(args);
        env.json(&a)
            .iter()
            .map(|n| n["text"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(textes(&["--since", "2026-02-01"]), ["b", "c"]);
    assert_eq!(textes(&["--until", "2026-02-01"]), ["a", "b"]);
    assert_eq!(textes(&["-t", "x"]), ["a", "b"]);
    assert_eq!(textes(&["-t", "x", "-t", "y"]), ["b"]);
    assert_eq!(textes(&["-c", "PROJETS"]), ["a", "c"]);
    assert_eq!(textes(&["-n", "1"]), ["c"]);

    env.note()
        .args(["list", "--since", "n'importe"])
        .assert()
        .failure()
        .stderr(contains("non comprise"));
    env.note()
        .args(["list", "--since", "3x"])
        .assert()
        .failure()
        .stderr(contains("unité inconnue"));
}

#[test]
fn list_tolere_une_ligne_corrompue() {
    let env = Env::new();
    env.take(&["bonne"]);
    let f = env.path().join("notes.jsonl");
    let mut contenu = std::fs::read_to_string(&f).unwrap();
    contenu.push_str("pas du json\n");
    std::fs::write(&f, contenu).unwrap();
    env.note()
        .arg("list")
        .assert()
        .success()
        .stdout(contains("bonne"))
        .stderr(contains("ligne 2 ignorée"));
}

// ---------------------------------------------------------------- search

#[test]
fn search_dans_texte_titre_tags_categorie() {
    let env = Env::new();
    env.take(&["Le client veut un EXPORT"]);
    env.take(&["rien", "--title", "Réunion budget"]);
    env.take(&["rien", "-t", "facturation"]);
    env.take(&["rien", "-c", "Ventes"]);
    env.take(&["sans rapport"]);

    let n = |motif: &str| env.json(&["search", motif, "--json"]).len();
    assert_eq!(n("export"), 1);
    assert_eq!(n("BUDGET"), 1);
    assert_eq!(n("factur"), 1);
    assert_eq!(n("ventes"), 1);
    assert_eq!(n("rien"), 3);

    env.note()
        .args(["search", "client", "--full"])
        .assert()
        .success()
        .stdout(contains("Le client veut un EXPORT"));
    env.note()
        .args(["search", "introuvable"])
        .assert()
        .success()
        .stderr(contains("aucune note"));
}

// ---------------------------------------------------------------- show

#[test]
fn show_note_complete() {
    let env = Env::new();
    let id = env.take(&["corps", "-c", "RH", "-t", "a,b", "--title", "T"]);
    env.note()
        .args(["show", &id])
        .assert()
        .success()
        .stdout(contains("titre      T"))
        .stdout(contains("catégorie  RH"))
        .stdout(contains("tags       a, b"))
        .stdout(contains("\ncorps\n"));
}

#[test]
fn show_sans_metadonnees_et_id_inconnu() {
    let env = Env::new();
    let id = env.take(&["corps"]);
    env.note()
        .args(["show", &id[..4]])
        .assert()
        .success()
        .stdout(contains("catégorie  —"))
        .stdout(contains("tags       —"))
        .stdout(contains("titre").not());
    env.note()
        .args(["show", "zzzz"])
        .assert()
        .failure()
        .stderr(contains("aucune note ne correspond"));
}

// ---------------------------------------------------------------- edit

#[test]
fn edit_texte_et_metadonnees() {
    let env = Env::new();
    let id = env.take(&["avant", "-c", "RH", "-t", "a"]);

    env.note()
        .args(["edit", &id, "--text", "après", "--title", "Titre"])
        .assert()
        .success()
        .stdout(contains("modifiée"));
    env.note()
        .args(["edit", &id, "--add-tag", "b,a"])
        .assert()
        .success();
    let n = &env.all()[0];
    assert_eq!(n["text"], "après");
    assert_eq!(n["title"], "Titre");
    assert_eq!(n["tags"], serde_json::json!(["a", "b"]));

    env.note()
        .args(["edit", &id, "-c", "", "--title", " ", "-t", "z"])
        .assert()
        .success();
    let n = &env.all()[0];
    assert_eq!(n["text"], "après");
    assert!(n.get("category").is_none());
    assert!(n.get("title").is_none());
    assert_eq!(n["tags"], serde_json::json!(["z"]));

    env.note()
        .args(["edit", &id, "-c", "Ventes"])
        .assert()
        .success();
    assert_eq!(env.all()[0]["category"], "Ventes");
}

#[test]
fn edit_via_editeur() {
    let env = Env::new();
    let id = env.take(&["avant"]);
    let ed = env.editeur(r#"printf 'réécrit\n\n' > "$1""#);
    env.note()
        .args(["edit", &id])
        .env("EDITOR", &ed)
        .assert()
        .success();
    assert_eq!(env.all()[0]["text"], "réécrit");
}

#[test]
fn edit_via_editeur_texte_vide_annule() {
    let env = Env::new();
    let id = env.take(&["avant"]);
    let ed = env.editeur(r#": > "$1""#);
    env.note()
        .args(["edit", &id])
        .env("VISUAL", &ed)
        .assert()
        .failure()
        .stderr(contains("texte vide"));
    assert_eq!(env.all()[0]["text"], "avant");
}

#[test]
fn edit_editeur_en_erreur_ou_introuvable() {
    let env = Env::new();
    let id = env.take(&["avant"]);
    let ed = env.editeur("exit 3");
    env.note()
        .args(["edit", &id])
        .env("EDITOR", &ed)
        .assert()
        .failure()
        .stderr(contains("terminé en erreur"));
    env.note()
        .args(["edit", &id])
        .env("EDITOR", env.path().join("inexistant"))
        .assert()
        .failure()
        .stderr(contains("lancement de l'éditeur"));
}

// ---------------------------------------------------------------- rm

#[test]
fn rm_force() {
    let env = Env::new();
    let id = env.take(&["à supprimer"]);
    env.take(&["à garder"]);
    env.note()
        .args(["rm", &id, "--force"])
        .assert()
        .success()
        .stdout(contains("supprimée"));
    let notes = env.all();
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0]["text"], "à garder");
}

#[test]
fn rm_sans_force_hors_terminal_refuse() {
    let env = Env::new();
    let id = env.take(&["x"]);
    env.note()
        .args(["remove", &id])
        .write_stdin("o\n")
        .assert()
        .failure()
        .stderr(contains("--force"));
    assert_eq!(env.all().len(), 1);
}

// ---------------------------------------------------------------- tags / cats

#[test]
fn tags_et_cats_vides() {
    let env = Env::new();
    env.note()
        .arg("tags")
        .assert()
        .success()
        .stderr(contains("aucun tag"));
    env.note()
        .arg("cats")
        .assert()
        .success()
        .stderr(contains("aucune catégorie"));
}

#[test]
fn tags_et_cats_tries_par_frequence() {
    let env = Env::new();
    env.take(&["1", "-t", "zeta,alpha", "-c", "RH"]);
    env.take(&["2", "-t", "zeta,beta", "-c", "Projets"]);
    env.take(&["3", "-c", "Projets"]);
    env.note()
        .arg("tags")
        .assert()
        .success()
        .stdout("zeta   2\nalpha  1\nbeta   1\n");
    env.note()
        .arg("categories")
        .assert()
        .success()
        .stdout("Projets  2\nRH       1\n");
}

// ---------------------------------------------------------------- export

#[test]
fn export_frise_par_defaut() {
    let env = Env::new();
    env.take(&["a", "--at", "2026-06-08T09:00", "-c", "Projets"]);
    env.take(&["b", "--at", "2026-06-08T14:00", "-c", "Projets"]);
    let ev = env.json(&["export"]);
    assert_eq!(ev.len(), 2);
    assert_eq!(ev[0]["date_start"], "2026-06-08");
    assert_eq!(ev[0]["time_of_day"], "09h00");

    let ev = env.json(&["export", "--format", "frise", "--group-by-day"]);
    assert_eq!(ev.len(), 1);
    assert_eq!(ev[0]["content"], serde_json::json!(["a", "b"]));
}

#[test]
fn export_json_et_md() {
    let env = Env::new();
    env.take(&["contenu", "--at", "2026-06-08T09:00"]);
    let notes = env.json(&["export", "--format", "json"]);
    assert_eq!(notes[0]["text"], "contenu");
    env.note()
        .args(["export", "--format", "md"])
        .assert()
        .success()
        .stdout(contains("# Notes"))
        .stdout(contains("## 2026-06-08"))
        .stdout(contains("### 09h00 — contenu"));
}

#[test]
fn export_vers_fichier_cree_les_dossiers() {
    let env = Env::new();
    env.take(&["a"]);
    let sortie = env.path().join("sous/dossier/frise.json");
    env.note()
        .args(["export", "-o"])
        .arg(&sortie)
        .assert()
        .success()
        .stdout("")
        .stderr(contains("1 note(s) écrite(s)"));
    let v: Value = serde_json::from_str(&std::fs::read_to_string(&sortie).unwrap()).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
}

#[test]
fn export_vers_fichier_relatif() {
    let env = Env::new();
    env.take(&["a"]);
    env.note()
        .current_dir(env.path())
        .args(["export", "--format", "md", "-o", "notes.md"])
        .assert()
        .success();
    assert!(env.path().join("notes.md").exists());
}

// ---------------------------------------------------------------- divers

#[test]
fn path_suit_note_dir() {
    let env = Env::new();
    let attendu = format!("{}\n", env.path().join("notes.jsonl").display());
    env.note().arg("path").assert().success().stdout(attendu);
}

#[test]
fn completions_bash() {
    let env = Env::new();
    env.note()
        .args(["completions", "bash"])
        .assert()
        .success()
        .stdout(contains("_note"));
}

#[test]
fn sous_commande_inconnue() {
    let env = Env::new();
    env.note().arg("inconnue").assert().failure().code(2);
}
