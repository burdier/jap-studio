//! Publicación del índice Lua ya validado; abrir el buscador nunca lee paquetes.
use super::Palette;
use elevate::catalog::tools::ToolCatalogEntry;

impl Palette {
    pub(in crate::ui) fn tool_entries(&self) -> &[ToolCatalogEntry] {
        &self.tools
    }

    pub(in crate::ui) fn replace_tools(&mut self, tools: Vec<ToolCatalogEntry>) {
        let selected = self
            .results
            .get(self.selected)
            .map(|entry| entry.id.clone());
        self.tools = tools;
        self.refresh();
        if let Some(index) = self
            .results
            .iter()
            .position(|entry| Some(&entry.id) == selected.as_ref())
        {
            self.selected = index;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use elevate::{
        actions::ActionSpec,
        catalog::{self, CatalogAction, EntryKind, OwnedCatalogEntry},
    };

    fn tool() -> ToolCatalogEntry {
        ToolCatalogEntry {
            entry: OwnedCatalogEntry {
                id: "lua.tool.user.accent".into(),
                kind: EntryKind::Action,
                name: "Acentuación propia".into(),
                summary: "Herramienta del usuario".into(),
                keywords: vec!["lua".into()],
                action: CatalogAction::Invoke("midi.tool user.accent".into()),
            },
            spec: ActionSpec {
                command: "midi.tool user.accent".into(),
                description: "Herramienta del usuario".into(),
                parameters: vec![],
            },
        }
    }

    #[test]
    fn published_tools_share_search_categories_and_keyboard_selection() {
        let mut palette = Palette::new();
        palette.replace_tools(vec![tool()]);
        palette.open_for_track(0);
        assert_eq!(palette.results().len(), catalog::entries().len() + 1);
        palette.set_category(Some(EntryKind::Action));
        for key in "acentuacion".chars() {
            palette.handle_key(&key.to_string(), true);
        }
        assert_eq!(palette.results().len(), 1);
        let chosen = palette.handle_key("enter", true).unwrap();
        assert!(chosen.is_tool);
        assert!(!chosen.has_parameters);
        assert_eq!(
            chosen.action,
            CatalogAction::Invoke("midi.tool user.accent".into())
        );
        palette.set_category(Some(EntryKind::Instrument));
        assert!(palette.results().is_empty());
        palette.set_category(Some(EntryKind::Action));
        palette.replace_tools(vec![]);
        assert!(palette.results().is_empty());
        palette.open_for_track(0);
        assert_eq!(palette.results().len(), catalog::entries().len());
    }

    #[test]
    fn suggestions_follow_published_tools_and_keep_native_commands() {
        use crate::ui::command_suggestions::completions;
        let tools = vec![tool()];
        let results = completions("midi.tool user.accent", &tools);
        assert_eq!(
            results[0],
            ("midi.tool user.accent".into(), "Acentuación propia".into())
        );
        assert!(completions("midi.tool user.accent", &[]).is_empty());
        assert!(
            completions("play", &tools)
                .iter()
                .any(|(command, _)| command == "play")
        );
        assert!(
            completions("midi.tools.reload", &[])
                .iter()
                .any(|(command, _)| command == "midi.tools.reload")
        );
    }
}
