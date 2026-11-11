//! Metadatos estáticos o con propietario; sin instancias de plugin ni VM.
use super::{EntryKind, normalize};

/// Acción provisional identificada por un comando estable. El host de acciones
/// sustituirá estos strings cuando exista; el catálogo no importa la UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CatalogAction<S = &'static str> {
    Invoke(S),
    Unavailable(S),
}

impl<S: AsRef<str>> CatalogAction<S> {
    /// Some declara que la invocación visual debe recoger argumentos antes de ejecutarse.
    pub fn parameters(&self) -> Option<&'static crate::actions::ActionSpec> {
        match self {
            Self::Invoke(command) => crate::actions::find(command.as_ref()),
            Self::Unavailable(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CatalogEntry<S = &'static str, K = &'static [&'static str]> {
    pub id: S,
    pub kind: EntryKind,
    pub name: S,
    pub summary: S,
    pub keywords: K,
    pub action: CatalogAction<S>,
}

pub type OwnedCatalogEntry = CatalogEntry<String, Vec<String>>;

impl<S: AsRef<str>, K: AsRef<[S]>> CatalogEntry<S, K> {
    pub fn matches(&self, query: &str) -> bool {
        self.match_score(&normalize(query)).is_some()
    }

    pub(super) fn match_score(&self, normalized_query: &str) -> Option<u8> {
        let name = normalize(self.name.as_ref());
        if name.starts_with(normalized_query) {
            return Some(0);
        }
        if name
            .split_whitespace()
            .any(|word| word.starts_with(normalized_query))
        {
            return Some(1);
        }
        if name.contains(normalized_query) {
            return Some(2);
        }
        let haystack = normalize(&format!(
            "{} {} {} {}",
            self.id.as_ref(),
            self.summary.as_ref(),
            self.keywords
                .as_ref()
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<_>>()
                .join(" "),
            self.kind.label()
        ));
        haystack.contains(normalized_query).then_some(3)
    }
}

impl<S: AsRef<str>> CatalogAction<S> {
    pub fn to_owned(&self) -> CatalogAction<String> {
        match self {
            Self::Invoke(command) => CatalogAction::Invoke(command.as_ref().into()),
            Self::Unavailable(reason) => CatalogAction::Unavailable(reason.as_ref().into()),
        }
    }
}
