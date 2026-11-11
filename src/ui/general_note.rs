//! Apunte general único, editable desde la acción `note.edit`.
use gpui::{
    AppContext, Context, Entity, FocusHandle, Focusable, Window, WindowId,
    component::input::TextareaState,
};
use std::{fs, io, path::PathBuf};

pub(super) struct GeneralNote {
    pub input: Entity<TextareaState>,
    pub focus: FocusHandle,
    pub error: Option<String>,
    pub window_id: WindowId,
    return_focus: Option<FocusHandle>,
}

fn path() -> io::Result<PathBuf> {
    dirs::data_dir()
        .map(|dir| dir.join("hay-daw/general-note.md"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "No se encontró la carpeta de datos del usuario",
            )
        })
}

fn read() -> io::Result<String> {
    match fs::read_to_string(path()?) {
        Ok(note) => Ok(note),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error),
    }
}

fn save(note: &str) -> io::Result<()> {
    let path = path()?;
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("Ruta del apunte inválida"))?;
    fs::create_dir_all(parent)?;
    fs::write(path, note)
}

impl super::DawApp {
    pub(in crate::ui) fn open_general_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let initial = match read() {
            Ok(note) => note,
            Err(error) => {
                self.status_message = format!("No se pudo abrir el apunte: {error}");
                return;
            }
        };
        let input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .default_value(initial)
                .auto_grow(8, 30)
        });
        let focus = input.read(cx).focus_handle(cx);
        let note = GeneralNote {
            input: input.clone(),
            focus,
            error: None,
            window_id: window.window_handle().window_id(),
            return_focus: window.focused(cx),
        };
        input.update(cx, |input, cx| input.focus(window, cx));
        self.general_note = Some(note);
        cx.notify();
    }

    pub(in crate::ui) fn close_general_note(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(note) = self.general_note.take() {
            window.focus(note.return_focus.as_ref().unwrap_or(&self.focus_handle), cx);
            cx.notify();
        }
    }

    pub(in crate::ui) fn save_general_note(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(note) = self.general_note.as_ref() else {
            return;
        };
        let content = note.input.read(cx).value().to_string();
        match save(&content) {
            Ok(()) => {
                self.close_general_note(window, cx);
                self.status_message = "Apunte guardado".into();
            }
            Err(error) => {
                if let Some(note) = self.general_note.as_mut() {
                    note.error = Some(format!("No se pudo guardar el apunte: {error}"));
                }
            }
        }
        cx.notify();
    }
}
