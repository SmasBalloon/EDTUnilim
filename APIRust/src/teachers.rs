//! Les enseignants, et leurs écritures multiples.
//!
//! Le PDF nomme la même personne « Dubreuil L. » dans une grande case et « LD »
//! dans une petite. Ce module ramène les deux à la même identité : un
//! identifiant stable et un nom unique, pour que l'emploi du temps d'un
//! enseignant se cherche d'une seule façon.
//!
//! Le fichier est embarqué dans le binaire : il change quand l'équipe change,
//! pas au fil des semaines.

use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use serde::Serialize;

const TEACHERS_CSV: &str = include_str!("../data/teachers.csv");

/// Un enseignant du référentiel.
#[derive(Debug, Clone, Serialize)]
pub struct Teacher {
    /// Identifiant stable, dérivé du nom : `dubreuil-l`.
    pub id: String,
    /// Nom tel qu'il s'affiche : `Dubreuil L.`.
    pub name: String,
    /// Autres écritures déclarées, initiales devinées comprises.
    pub aliases: Vec<String>,
}

/// Une mention d'enseignant rattachée à quelqu'un.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub id: String,
    pub name: String,
}

/// Le référentiel, analysé une seule fois pour toute la durée du process.
pub fn entries() -> &'static [Teacher] {
    &registry().teachers
}

/// À qui renvoie une mention lue dans un PDF.
///
/// Rend `None` pour ce qui n'est pas quelqu'un (un libellé de groupe tombé dans
/// la case, une mention vide). Une écriture inconnue n'est pas perdue pour
/// autant : elle devient une identité à elle seule, sous son propre nom.
pub fn resolve(printed: &str) -> Option<Named> {
    registry().resolve(printed)
}

struct Registry {
    teachers: Vec<Teacher>,
    /// Écriture repliée → rang de l'enseignant.
    by_key: HashMap<String, usize>,
    /// Écritures repliées qui ne désignent personne.
    ignored: HashSet<String>,
}

impl Registry {
    fn resolve(&self, printed: &str) -> Option<Named> {
        let printed = printed.trim();
        let key = fold(printed);
        if key.is_empty() || self.ignored.contains(&key) {
            return None;
        }
        match self.by_key.get(&key) {
            Some(index) => {
                let teacher = &self.teachers[*index];
                Some(Named { id: teacher.id.clone(), name: teacher.name.clone() })
            }
            None => Some(Named { id: slug(printed), name: printed.to_owned() }),
        }
    }
}

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| build(TEACHERS_CSV))
}

fn build(csv: &str) -> Registry {
    let mut teachers: Vec<Teacher> = Vec::new();
    let mut ignored: HashSet<String> = HashSet::new();

    for line in csv.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (name, aliases) = line.split_once(';').unwrap_or((line, ""));
        let name = name.trim();
        let aliases: Vec<String> =
            aliases.split('|').map(str::trim).filter(|alias| !alias.is_empty()).map(str::to_owned).collect();

        if name == "-" {
            ignored.extend(aliases.iter().map(|alias| fold(alias)));
        } else if !name.is_empty() {
            teachers.push(Teacher { id: slug(name), name: name.to_owned(), aliases });
        }
    }

    // Les écritures déclarées d'abord : elles font autorité sur les initiales
    // devinées, qui ne doivent jamais reprendre une graphie déjà attribuée.
    let mut by_key: HashMap<String, usize> = HashMap::new();
    let mut claimed: HashSet<String> = ignored.clone();
    for (index, teacher) in teachers.iter().enumerate() {
        for written in std::iter::once(&teacher.name).chain(teacher.aliases.iter()) {
            let key = fold(written);
            claimed.insert(key.clone());
            by_key.insert(key, index);
        }
    }

    // Puis les initiales déduites du nom, écartées dès que deux enseignants les
    // revendiquent : mieux vaut deux inconnus qu'une personne pour une autre.
    let mut guessed: HashMap<String, Option<usize>> = HashMap::new();
    for (index, teacher) in teachers.iter().enumerate() {
        let Some(initials) = initials_of(&teacher.name) else { continue };
        let key = fold(&initials);
        if claimed.contains(&key) {
            continue;
        }
        guessed.entry(key).and_modify(|slot| *slot = None).or_insert(Some(index));
    }
    for (key, owner) in guessed {
        if let Some(index) = owner {
            teachers[index].aliases.push(key.to_uppercase());
            by_key.insert(key, index);
        }
    }

    for teacher in &mut teachers {
        teacher.aliases.sort();
        teacher.aliases.dedup();
    }
    Registry { teachers, by_key, ignored }
}

/// `Dubreuil L.` → `LD` : l'initiale du prénom, puis celle du nom.
///
/// C'est ainsi que le générateur abrège quand la case est étroite. Un nom qui
/// ne suit pas la forme « Nom P. » n'a pas d'initiales devinables.
fn initials_of(name: &str) -> Option<String> {
    let (surname, given) = name.trim().rsplit_once(' ')?;
    let given = given.trim_end_matches('.');
    if given.chars().count() != 1 {
        return None;
    }
    let first = surname.chars().next()?;
    Some(format!("{}{}", given.to_uppercase(), first.to_uppercase()))
}

/// Replie une écriture pour la comparaison : sans casse, sans accents, sans
/// ponctuation ni espaces. `Mérillou N.` et `merillou n` se rejoignent.
fn fold(text: &str) -> String {
    text.chars().filter_map(simplify).collect()
}

/// Identifiant lisible dans une URL : `Dubreuil L.` → `dubreuil-l`.
fn slug(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for letter in text.chars() {
        match simplify(letter) {
            Some(plain) => out.push(plain),
            None if !out.ends_with('-') => out.push('-'),
            None => {}
        }
    }
    out.trim_matches('-').to_owned()
}

/// Une lettre ramenée à sa forme nue, ou rien si ce n'en est pas une.
fn simplify(letter: char) -> Option<char> {
    // La casse tombe d'abord : « É » doit suivre le même chemin que « é ».
    let plain = match letter.to_lowercase().next()? {
        'à' | 'â' | 'ä' | 'á' | 'å' | 'ã' => 'a',
        'ç' => 'c',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ñ' => 'n',
        'ò' | 'ó' | 'ô' | 'ö' | 'õ' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'œ' => 'o',
        'æ' => 'a',
        other if other.is_alphanumeric() => other,
        _ => return None,
    };
    Some(plain)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Un référentiel d'essai : les tests de mécanique ne dépendent pas du
    /// fichier livré, qui se modifie à la main au fil des arrivées et départs.
    const SAMPLE: &str = "\
# une équipe pour l'exemple
Dubreuil L.
Bombeaux C.;BX
Mérillou N.
Mérillou S.
-;G4|Indéterminé I.
";

    #[test]
    fn full_name_and_initials_meet() {
        let registry = build(SAMPLE);
        let full = named(&registry, "Dubreuil L.").expect("un enseignant");
        let short = named(&registry, "LD").expect("le même enseignant");
        assert_eq!(full, short);
        assert_eq!(full.id, "dubreuil-l");
        assert_eq!(full.name, "Dubreuil L.");
    }

    #[test]
    fn declared_aliases_win() {
        let registry = build(SAMPLE);
        assert_eq!(named(&registry, "BX").map(|who| who.name), Some("Bombeaux C.".into()));
    }

    #[test]
    fn accents_and_case_do_not_matter() {
        let registry = build(SAMPLE);
        let expected = named(&registry, "Mérillou N.");
        assert_eq!(named(&registry, "merillou n."), expected);
        assert_eq!(named(&registry, "MÉRILLOU N."), expected);
        assert_eq!(named(&registry, "nm"), expected);
    }

    #[test]
    fn namesakes_keep_their_distance() {
        let registry = build(SAMPLE);
        assert_ne!(named(&registry, "Mérillou N."), named(&registry, "Mérillou S."));
        assert_eq!(named(&registry, "SM").map(|who| who.name), Some("Mérillou S.".into()));
    }

    #[test]
    fn group_labels_are_not_people() {
        let registry = build(SAMPLE);
        assert!(named(&registry, "G4").is_none());
        assert!(named(&registry, "Indéterminé I.").is_none());
        assert!(named(&registry, "   ").is_none());
    }

    #[test]
    fn unknown_writings_stand_on_their_own() {
        let registry = build(SAMPLE);
        let unknown = named(&registry, "GAC").expect("une identité à elle seule");
        assert_eq!(unknown.id, "gac");
        assert_eq!(unknown.name, "GAC");
    }

    #[test]
    fn shared_initials_are_left_alone() {
        let registry = build("Dupont A.\nDurand A.\nMartin B.");
        assert!(!registry.by_key.contains_key(&fold("AD")));
        assert!(registry.by_key.contains_key(&fold("BM")));
    }

    #[test]
    fn a_declared_writing_blocks_the_guess() {
        // « TB » est déclaré ailleurs : il ne devient pas l'abréviation de
        // Berthier T. par ricochet.
        let registry = build("Berthier T.\nTruc B.;TB");
        assert_eq!(registry.by_key.get(&fold("TB")).copied(), Some(1));
    }

    #[test]
    fn builds_readable_identifiers() {
        assert_eq!(slug("Mérillou N."), "merillou-n");
        assert_eq!(slug("O'Neil  P."), "o-neil-p");
        assert_eq!(slug("--"), "");
    }

    /// Le fichier livré se corrige à la main : ce test veille sur sa forme.
    /// Un « : » à la place du « ; » ferait un nom à rallonge sans rien dire.
    #[test]
    fn shipped_registry_is_sound() {
        assert!(entries().len() >= 10, "le référentiel semble vide");

        let mut ids: Vec<&str> = Vec::new();
        for teacher in entries() {
            assert!(
                !teacher.name.contains(':') && !teacher.name.contains(';'),
                "« {} » : le nom et ses alias se séparent par un point-virgule",
                teacher.name,
            );
            assert!(!teacher.id.is_empty(), "« {} » n'a pas d'identifiant", teacher.name);
            ids.push(&teacher.id);
        }
        ids.sort_unstable();
        let unique = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), unique, "deux enseignants portent le même identifiant");
    }

    #[test]
    fn guesses_initials_only_from_the_usual_shape() {
        assert_eq!(initials_of("Mérillou N."), Some("NM".to_owned()));
        assert_eq!(initials_of("Dubreuil L"), Some("LD".to_owned()));
        assert_eq!(initials_of("Dubreuil"), None, "pas de prénom");
        assert_eq!(initials_of("Dubreuil Louis"), None, "prénom en toutes lettres");
    }

    #[test]
    fn folds_every_accented_letter_to_its_plain_form() {
        assert_eq!(fold("àâäáåã ç èéêë ìíîï ñ òóôöõ ùúûü ýÿ œ æ"), "aaaaaaceeeeiiiinooooouuuuyyoa");
        assert_eq!(fold("É-L'a"), "ela");
        assert_eq!(slug("Œuvre  É."), "ouvre-e");
    }

    #[test]
    fn resolves_through_the_shipped_registry() {
        assert!(resolve("   ").is_none());
        let stranger = resolve("Inconnu Z.").expect("une identité à part");
        assert_eq!(stranger.id, "inconnu-z");
        assert_eq!(stranger.name, "Inconnu Z.");
    }

    /// Résout une écriture dans un référentiel d'essai.
    fn named(registry: &Registry, printed: &str) -> Option<Named> {
        registry.resolve(printed)
    }
}
