use super::{Explorer, ExplorerEvent, ExplorerStyle};
use crate::ui::{
    components::tree::{self, ADD as ADD_FOLDER, AUDIO, CLOSE, FILE, FOLDER, TreeRow},
    icons::render_svg_icon,
};
use elevate::explorer::EntryKind;
use gpui::{
    Context, Div, IntoElement, MouseButton, MouseDownEvent, Render, Stateful, Window,
    component::input::Input, div, prelude::*, px, rgb,
};

use std::path::PathBuf;

const PREVIEW_PLAY: &[u8] =
    include_bytes!("../../../assets/icons/toolbar/media-playback-start.svg");
const PREVIEW_STOP: &[u8] = include_bytes!("../../../assets/icons/toolbar/media-playback-stop.svg");
const HEADPHONES: &[u8] = include_bytes!("../../../assets/icons/toolbar/headphones.svg");
const MIDI: &[u8] = include_bytes!("../../../assets/icons/toolbar/piano.svg");

#[derive(Clone)]
pub(in crate::ui) struct ExplorerSampleDrag {
    pub path: PathBuf,
    style: ExplorerStyle,
}

impl Render for ExplorerSampleDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let icon = if EntryKind::for_file(&self.path) == EntryKind::Hydrogen {
            FILE
        } else {
            AUDIO
        };
        div()
            .flex()
            .items_center()
            .gap_2()
            .max_w(px(260.0))
            .px_2()
            .py_1()
            .text_xs()
            .rounded(px(3.0))
            .bg(rgb(self.style.selected))
            .text_color(rgb(self.style.text))
            .child(render_svg_icon(icon, 13.0, self.style.accent))
            .child(
                div().truncate().child(
                    self.path
                        .file_name()
                        .unwrap_or(self.path.as_os_str())
                        .to_string_lossy()
                        .into_owned(),
                ),
            )
    }
}

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.init_search_input(window, cx);
        let style = self.style;
        let empty = self.tree.roots().is_empty();
        let message = self.save_error.as_ref().or(self.status.as_ref()).cloned();
        let selected_status = self
            .visible_rows()
            .iter()
            .find(|row| Some(&row.entry.path) == self.selected.as_ref())
            .and_then(|row| row.status.clone());
        tree::panel("library-explorer", style)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, event, window, cx| this.handle_key(event, window, cx)))
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
            .when(!empty, |panel| {
                panel
                    .when_some(self.search.input.as_ref(), |panel, input| panel.child(
                        div().px_2().pb_2().flex_shrink_0()
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .child(Input::new(input).w_full().cleanable(true)),
                    ))
                    .child(self.render_preview_controls(cx))
                    .child(self.render_sample_profile())
                    .child(self.render_root_controls(cx))
                    .when_some(self.search_message(), |panel, text| panel.child(
                        div().px_2().pb_1().text_color(rgb(style.muted)).child(text),
                    ))
                    .child(tree::list("library-tree", self.visible_rows().len(), &self.scroll, cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                        range.map(|index| this.render_row(index, cx)).collect()
                    })))
            })
            .child(
                tree::footer(style)
                    .when_some(message, |footer, text| footer.child(div().text_color(rgb(style.accent)).child(text)))
                    .when(self.saving, |footer| footer.child("Guardando bibliotecas…"))
                    .when_some(selected_status, |footer, status| footer.child(status))
                    .when_some(self.selected.as_ref(), |footer, path| footer.child(
                        div().truncate().child(path.to_string_lossy().into_owned()),
                    ))
                    .child("Arrastra audio a Drums Latex")
                    .child("Doble clic / Enter: insertar en pista activa")
                    .child("→ Preescuchar / repetir · Ctrl+F: buscar")
                    .child("En el cursor · e / Esc: cerrar"),
            )
    }
}

impl Explorer {
    fn render_sample_profile(&self) -> Div {
        let style = self.style;
        let Some(path) = self
            .selected_file()
            .filter(|p| EntryKind::for_file(p) == EntryKind::Audio)
        else {
            return div();
        };
        let card = div()
            .flex_shrink_0()
            .mx_2()
            .mb_2()
            .px_2()
            .py_1()
            .rounded(px(style.radius))
            .bg(rgb(style.hover))
            .flex()
            .flex_col()
            .gap_1();

        let Some(p) = self.selected_profile(&path) else {
            return if self.preview.profile_loading {
                card.child(
                    div()
                        .text_xs()
                        .text_color(rgb(style.muted))
                        .child("Analizando carácter acústico…"),
                )
            } else {
                div()
            };
        };

        let chip = |label: String, color: u32| {
            div()
                .px_1()
                .rounded(px(3.0))
                .bg(rgb(style.panel))
                .text_xs()
                .text_color(rgb(color))
                .child(label)
        };

        let transient_color = if p.transient_strength < 0.35 {
            style.muted
        } else {
            style.accent
        };

        let band_bar = |label: &'static str, pct: f32| {
            let filled = (pct / 100.0).clamp(0.04, 1.0);
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .text_xs()
                        .text_color(rgb(style.muted))
                        .child(label)
                        .child(format!("{pct:.0}%")),
                )
                .child(
                    div()
                        .w_full()
                        .h(px(3.0))
                        .rounded(px(1.5))
                        .bg(rgb(style.panel))
                        .child(
                            div()
                                .h_full()
                                .w(gpui::relative(filled))
                                .rounded(px(1.5))
                                .bg(rgb(style.accent)),
                        ),
                )
        };

        let mut tags = div()
            .flex()
            .flex_wrap()
            .gap_1()
            .child(chip(format!("{} · {:.2}s", p.role_hint, p.duration_seconds), style.text))
            .child(chip(
                format!("Transiente: {} ({:.0}ms)", p.transient_label, p.attack_ms),
                transient_color,
            ))
            .child(chip(
                format!("{} ({:.0} Hz)", p.brightness_label, p.spectral_centroid_hz),
                style.muted,
            ));

        if let Some(note) = &p.pitch_note {
            tags = tags.child(chip(format!("Tono {note}"), style.accent));
        }

        card.child(tags)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .pt(px(2.0))
                    .child(band_bar("Sub", p.bands.sub_pct))
                    .child(band_bar("Grv", p.bands.low_pct))
                    .child(band_bar("Med", p.bands.mid_pct))
                    .child(band_bar("Agd", p.bands.high_pct)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(style.muted))
                    .child(format!(
                        "Decay {:.0} ms · Crest {:.1} dB · Pico {:.1} dBFS",
                        p.decay_ms, p.crest_db, p.peak_dbfs
                    )),
            )
    }

    fn render_root_controls(&self, cx: &mut Context<Self>) -> Div {
        let container = div()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .px_2()
            .pb_2();
        let Some(path) = self
            .selected_file()
            .filter(|p| EntryKind::for_file(p) == EntryKind::Audio)
        else {
            return container;
        };
        let root = self.selected_root(&path);
        let chosen = root.map_or((60, 0), |value| value.chosen);
        let name = elevate::musical::pitch::note_name(chosen.0);
        let description = match root {
            Some(root) if root.detected == Some(chosen) => {
                format!("Raíz ≈ {name} {:+} ¢", chosen.1)
            }
            Some(root) if root.detected.is_none() => format!("Sin tono estable · Raíz {name}"),
            _ => format!("Raíz manual · {name}"),
        };
        let detect_label = if self.preview.analysing.is_some() {
            "Analizando…"
        } else {
            "Detectar raíz"
        };
        let detect_path = path.clone();
        let lower_path = path.clone();
        let higher_path = path.clone();
        let load_path = path.clone();
        let hilo_path = path.clone();
        let phrase_label = if self.preview.phrase && self.preview.path.as_ref() == Some(&path) {
            "Detener frase"
        } else {
            "Preescuchar con la frase"
        };
        let style = self.style;
        let button = |label: &'static str| {
            div()
                .px_1()
                .py_1()
                .cursor_pointer()
                .text_color(rgb(style.accent))
                .hover(move |s| s.bg(rgb(style.selected)))
                .child(label)
        };
        container
            .child(div().text_color(rgb(style.text)).child(description))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap_1()
                    .child(button(detect_label).id("detect-root").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.detect_root(detect_path.clone(), cx);
                            cx.stop_propagation();
                        }),
                    ))
                    .child(button("−").id("root-down").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.step_root(lower_path.clone(), -1, cx);
                            cx.stop_propagation();
                        }),
                    ))
                    .child(button("+").id("root-up").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.step_root(higher_path.clone(), 1, cx);
                            cx.stop_propagation();
                        }),
                    ))
                    .child(button("Usar como raíz").id("load-root").on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.stop_preview();
                            cx.emit(ExplorerEvent::LoadRoot(
                                load_path.clone(),
                                chosen.0,
                                chosen.1,
                            ));
                            cx.stop_propagation();
                        }),
                    )),
            )
            .when(self.has_phrase, |row| {
                row.child(
                    button("Añadir al Hilo")
                        .id("add-sample-hilo")
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.stop_preview();
                                cx.emit(ExplorerEvent::AddToHilo(
                                    hilo_path.clone(),
                                    chosen.0,
                                    chosen.1,
                                ));
                                cx.notify();
                                cx.stop_propagation();
                            }),
                        ),
                )
            })
            .when(self.has_phrase || self.preview.phrase, |row| {
                row.child(button(phrase_label).id("preview-phrase").on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _, _, cx| {
                        if this.preview.phrase && this.preview.path.as_ref() == Some(&path) {
                            this.stop_preview();
                            cx.notify();
                        } else {
                            cx.emit(ExplorerEvent::PreviewPhrase(
                                path.clone(),
                                chosen.0,
                                chosen.1,
                            ));
                        }
                        cx.stop_propagation();
                    }),
                ))
            })
    }

    fn render_preview_controls(&self, cx: &mut Context<Self>) -> Div {
        let style = self.style;
        let automatic = self.preview.automatic;
        let icon_color = if automatic { style.accent } else { style.muted };
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .px_2()
            .pb_1()
            .child(
                div()
                    .id("library-preview-auto")
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(22.0))
                    .px_1()
                    .rounded(px(style.radius))
                    .cursor_pointer()
                    .bg(rgb(if automatic {
                        style.selected
                    } else {
                        style.panel
                    }))
                    .hover(move |s| s.bg(rgb(style.hover)))
                    .text_color(rgb(icon_color))
                    .child(render_svg_icon(HEADPHONES, 13.0, icon_color))
                    .child("Auto")
                    .tooltip(|window, cx| {
                        gpui::component::tooltip::Tooltip::new(
                            "Preescuchar al seleccionar o navegar con las flechas",
                        )
                        .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_auto_preview(cx);
                        }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .ml_auto()
                    .child(div().text_color(rgb(style.muted)).child("Vol"))
                    .child(self.render_preview_level_button("−", -10, cx))
                    .child(
                        div()
                            .w(px(32.0))
                            .text_center()
                            .whitespace_nowrap()
                            .font_family("monospace")
                            .child(format!("{}%", self.preview.level)),
                    )
                    .child(self.render_preview_level_button("+", 10, cx)),
            )
    }

    fn render_preview_level_button(
        &self,
        label: &'static str,
        direction: i16,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let style = self.style;
        div()
            .id(("library-preview-volume", usize::from(direction > 0)))
            .size(px(20.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(style.radius))
            .bg(rgb(style.hover))
            .hover(move |s| s.bg(rgb(style.selected)))
            .cursor_pointer()
            .text_color(rgb(style.accent))
            .child(label)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _, _, cx| {
                    this.change_preview_level(direction, cx);
                }),
            )
    }

    fn render_row(&self, index: usize, cx: &mut Context<Self>) -> Stateful<Div> {
        let row = &self.visible_rows()[index];
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
            EntryKind::Other | EntryKind::Hydrogen => FILE,
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
        .when(
            matches!(row.entry.kind, EntryKind::Audio | EntryKind::Hydrogen),
            |line| {
                let drag = ExplorerSampleDrag {
                    path: row.entry.path.clone(),
                    style,
                };
                line.on_drag(drag, |drag, _, _, cx| cx.new(|_| drag.clone()))
            },
        )
        .when(row.entry.kind == EntryKind::Audio, |line| {
            let path = row.entry.path.clone();
            let active = self.preview.path.as_ref() == Some(&path);
            line.child(
                div()
                    .id(("library-preview-sample", index))
                    .ml_1()
                    .size(px(22.0))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(style.radius))
                    .bg(rgb(if active { style.selected } else { style.panel }))
                    .hover(move |s| s.bg(rgb(style.hover)))
                    .cursor_pointer()
                    .child(render_svg_icon(
                        if active { PREVIEW_STOP } else { PREVIEW_PLAY },
                        12.0,
                        style.accent,
                    ))
                    .tooltip(move |window, cx| {
                        gpui::component::tooltip::Tooltip::new(if active {
                            "Detener preescucha"
                        } else {
                            "Preescuchar muestra"
                        })
                        .build(window, cx)
                    })
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            if !this.focus.is_focused(window) {
                                cx.emit(ExplorerEvent::Focused);
                            }
                            window.focus(&this.focus, cx);
                            this.selected = Some(path.clone());
                            this.schedule_profile_analysis(Some(path.clone()), cx);
                            this.toggle_sample_preview(path.clone(), cx);
                            cx.stop_propagation();
                        }),
                    ),
            )
        })
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
