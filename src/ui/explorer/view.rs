use super::{Explorer, ExplorerEvent};
use crate::ui::{
    components::tree::{self, ADD as ADD_FOLDER, AUDIO, CLOSE, FILE, FOLDER, TreeRow},
    icons::render_svg_icon,
};
use elevate::explorer::EntryKind;
use gpui::{
    Context, Div, IntoElement, MouseButton, MouseDownEvent, Render, Stateful, Window, div,
    prelude::*, rgb,
};

const MIDI: &[u8] = include_bytes!("../../../assets/icons/toolbar/piano.svg");

impl Render for Explorer {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let style = self.style;
        let empty = self.tree.roots().is_empty();
        let message = self.save_error.as_ref().or(self.status.as_ref()).cloned();
        let selected_status = self
            .tree
            .rows()
            .iter()
            .find(|row| Some(&row.entry.path) == self.selected.as_ref())
            .and_then(|row| row.status.clone());
        tree::panel("library-explorer", style)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, _, cx| this.handle_key(event, cx)))
            .on_mouse_down(MouseButton::Left, cx.listener(|this, _, window, cx| {
                if !this.focus.is_focused(window) { cx.emit(ExplorerEvent::Focused); }
                window.focus(&this.focus, cx);
                cx.stop_propagation();
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .child(
                tree::header()
                    .child(div().flex_1().min_w_0().truncate().text_color(rgb(style.muted)).child("BIBLIOTECAS"))
                    .child(div().id("close-explorer").p_1().cursor_pointer()
                        .hover(move |s| s.bg(rgb(style.selected)))
                        .child(render_svg_icon(CLOSE, 13.0, style.muted))
                        .on_mouse_down(MouseButton::Left, cx.listener(|_, _, _, cx| {
                            cx.emit(ExplorerEvent::Close); cx.stop_propagation();
                        }))),
            )
            .child(
                div().flex_shrink_0().flex().items_center().gap_2().px_2().pb_2()
                    .child(div().id("add-library").flex_1().min_w_0().flex().items_center().gap_1()
                        .py_1().cursor_pointer().text_color(rgb(if self.ready && !self.picking { style.accent } else { style.muted }))
                        .child(render_svg_icon(ADD_FOLDER, 16.0, style.accent))
                        .child(div().truncate().child("Añadir biblioteca"))
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.add_library(cx))))
                    .child(div().id("refresh-libraries").py_1().cursor_pointer().text_color(rgb(style.muted))
                        .hover(move |s| s.text_color(rgb(style.text))).child("Recargar")
                        .on_mouse_down(MouseButton::Left, cx.listener(|this, _, _, cx| this.refresh(cx)))),
            )
            .when(empty, |panel| panel.child(
                div().flex_1().min_h_0().p_4().flex().flex_col().justify_center().gap_3()
                    .child(render_svg_icon(ADD_FOLDER, 28.0, style.accent))
                    .child("Tus sonidos, a mano")
                    .child(div().text_color(rgb(style.muted))
                        .child("Añade carpetas de audio o MIDI. Sus accesos quedarán guardados hasta que los quites."))
                    .child(div().text_color(rgb(style.muted)).child("Quitar una biblioteca no borra sus archivos.")),
            ))
            .when(!empty, |panel| panel.child(
                tree::list("library-tree", self.tree.rows().len(), &self.scroll, cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                    range.map(|index| this.render_row(index, cx)).collect()
                })),
            ))
            .child(
                tree::footer(style)
                    .when_some(message, |footer, text| footer.child(div().text_color(rgb(style.accent)).child(text)))
                    .when(self.saving, |footer| footer.child("Guardando bibliotecas…"))
                    .when_some(selected_status, |footer, status| footer.child(status))
                    .when_some(self.selected.as_ref(), |footer, path| footer.child(
                        div().truncate().child(path.to_string_lossy().into_owned()),
                    ))
                    .child("Doble clic / Enter: insertar en pista activa")
                    .child("En el cursor · e / Esc: cerrar"),
            )
    }
}

impl Explorer {
    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let row = &self.tree.rows()[index];
        let style = self.style;
        let selected = self.selected.as_ref() == Some(&row.entry.path);
        let color = if row.entry.kind == EntryKind::Other {
            style.muted
        } else {
            style.text
        };
        let icon = match row.entry.kind {
            EntryKind::Directory => FOLDER,
            EntryKind::Midi => MIDI,
            EntryKind::Audio => AUDIO,
            EntryKind::Other => FILE,
        };
        let remove_path = row.entry.path.clone();
        tree::row(
            row.entry.path.to_string_lossy().into_owned(),
            style,
            TreeRow {
                depth: row.depth,
                name: &row.entry.name,
                expanded: (row.entry.kind == EntryKind::Directory).then_some(row.expanded),
                selected,
                icon,
                color,
                icon_color: if row.entry.kind == EntryKind::Midi {
                    style.accent
                } else {
                    color
                },
                status: row.status.as_deref(),
            },
        )
        .when(row.root, |line| {
            line.child(
                div()
                    .id(("forget-library", index))
                    .ml_2()
                    .text_color(rgb(style.muted))
                    .hover(move |s| s.text_color(rgb(style.accent)))
                    .child("Quitar")
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            window.focus(&this.focus, cx);
                            this.remove_library(&remove_path, cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
        })
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                if event.click_count <= 2 {
                    this.activate_row(index, event.click_count == 2, cx);
                }
            }),
        )
    }
}
