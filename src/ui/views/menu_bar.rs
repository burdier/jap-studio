//! Accesos de la barra superior a los controladores de proyecto y vista.
use crate::ui::{
    CenterView, DawApp,
    components::{
        context_menu::{context_menu, menu_panel},
        modal::ModalClose,
    },
    toolbar::ToolbarAction,
    workspace_focus::Panel,
};
use gpui::{
    Context, Div, FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, Pixels, Window, div,
    point, prelude::*, px, rgb,
};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum MenuGroup {
    File,
    View,
}

pub(in crate::ui) struct BarMenu {
    group: MenuGroup,
    x: Pixels,
    selected: Option<usize>,
}

#[derive(Clone, Copy)]
enum MenuAction {
    Command(&'static str),
    Export,
    Fx,
    PianoRoll,
}

struct Item {
    label: &'static str,
    icon: &'static [u8],
    action: MenuAction,
    hint: &'static str,
}

macro_rules! item {
    ($label:literal, $icon:literal, $action:expr, $hint:literal) => {
        Item {
            label: $label,
            icon: include_bytes!(concat!("../../../assets/icons/toolbar/", $icon, ".svg")),
            action: $action,
            hint: $hint,
        }
    };
}
use MenuAction::{Command, Fx, PianoRoll};

impl MenuGroup {
    fn label(self) -> &'static str {
        match self {
            Self::File => "Archivo",
            Self::View => "Vista",
        }
    }

    fn items(self) -> &'static [Item] {
        match self {
            Self::File => &[
                item!(
                    "Abrir proyecto…",
                    "folder",
                    Command("project.open"),
                    "Ctrl+O"
                ),
                item!(
                    "Importar Hydrogen…",
                    "folder",
                    Command("hydrogen.import"),
                    ""
                ),
                item!(
                    "Guardar proyecto",
                    "document-save",
                    Command("project.save"),
                    "Ctrl+S"
                ),
                item!(
                    "Guardar como…",
                    "document-save-as",
                    Command("project.save_as"),
                    "Ctrl+Shift+S"
                ),
                item!(
                    "Mostrar carpeta del proyecto",
                    "folder-loupe",
                    Command("project.reveal"),
                    ""
                ),
                item!("Exportar audio…", "export", MenuAction::Export, ""),
                item!(
                    "Exportar MIDI (.mid)…",
                    "export",
                    Command("export.midi"),
                    ""
                ),
                #[cfg(feature = "p2p")]
                item!("Compartir sesión", "share", Command("project.share"), ""),
                #[cfg(feature = "p2p")]
                item!(
                    "Sincronizar sesión (Sync)",
                    "arrows-circle",
                    Command("project.sync"),
                    ""
                ),
                #[cfg(feature = "p2p")]
                item!(
                    "Unirse",
                    "chain-link",
                    Command("project.join"),
                    "Portapapeles"
                ),
                #[cfg(feature = "p2p")]
                item!(
                    "Salir de la sesión",
                    "connector",
                    Command("project.leave"),
                    ""
                ),
                #[cfg(feature = "p2p")]
                item!(
                    "Nombre e historial de edición…",
                    "addressbook",
                    Command("project.identity"),
                    ""
                ),
            ],
            Self::View => &[
                item!(
                    "Arrangement",
                    "view-continuous",
                    Command("arrangement"),
                    "F1"
                ),
                item!("Drums", "app-grid", Command("pattern"), "F2"),
                item!("Mixer", "mixer-sliders", Command("mixer"), "F3"),
                item!("Explorador", "folder", Command("explorer.toggle"), "Ctrl+E"),
                item!("Track FX", "sliders", Fx, ""),
                item!("Piano Roll", "piano", PianoRoll, ""),
                item!(
                    "Ampliar / restaurar panel activo",
                    "view-fullscreen",
                    Command("view.maximize"),
                    "F11"
                ),
            ],
        }
    }
}

impl DawApp {
    pub(super) fn render_bar_button(&self, group: MenuGroup, cx: &mut Context<Self>) -> Div {
        let ds = &self.ds;
        div()
            .px_2()
            .h_full()
            .flex()
            .items_center()
            .text_xs()
            .rounded(px(ds.radius_sm))
            .cursor_pointer()
            .hover(|button| button.bg(rgb(ds.panel_hover)))
            .when(
                self.bar_menu
                    .as_ref()
                    .is_some_and(|menu| menu.group == group),
                |button| button.bg(rgb(ds.panel_hover)).text_color(rgb(ds.accent)),
            )
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    this.bar_menu = if this
                        .bar_menu
                        .as_ref()
                        .is_some_and(|menu| menu.group == group)
                    {
                        None
                    } else {
                        Some(BarMenu {
                            group,
                            x: event.position.x - px(18.),
                            selected: None,
                        })
                    };
                    window.focus(&this.focus_handle, cx);
                    cx.stop_propagation();
                    cx.notify();
                }),
            )
            .child(group.label())
    }

    pub(in crate::ui) fn render_bar_menu(&self, window: &Window, cx: &mut Context<Self>) -> Div {
        let menu = self.bar_menu.as_ref().expect("menú abierto");
        let ds = &self.ds;
        let viewport = window.viewport_size();
        let width = 352.0_f32.min((f32::from(viewport.width) - 10.0).max(1.0));
        let top = px(ds.title_bar_height + 8.0);
        let x = menu
            .x
            .clamp(px(5.0), (viewport.width - px(width + 5.0)).max(px(5.0)));
        let on_close: ModalClose = Rc::new(cx.listener(|this, _, _, cx| {
            this.bar_menu = None;
            cx.notify();
        }));
        let list = div()
            .id("bar-menu-items")
            .max_h((viewport.height - top - px(14.0)).max(px(1.0)))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_1()
            .children(menu.group.items().iter().enumerate().map(|(index, item)| {
                let action = item.action;
                let checked = self.bar_action_checked(action);
                let active = menu.selected == Some(index);
                super::palette::palette_card(ds, ds.accent, active)
                    .flex_shrink_0()
                    .rounded(px(ds.radius_sm))
                    .on_mouse_move(cx.listener(move |this, _, _, cx| {
                        if let Some(menu) = &mut this.bar_menu
                            && menu.selected != Some(index)
                        {
                            menu.selected = Some(index);
                            cx.notify();
                        }
                    }))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, window, cx| {
                            this.run_bar_action(action, window, cx);
                            cx.stop_propagation();
                            cx.notify();
                        }),
                    )
                    .child(super::palette::palette_icon(
                        ds, item.icon, ds.accent, active,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .child(item.label),
                    )
                    .when(checked, |row| {
                        row.child(div().text_xs().text_color(rgb(ds.accent)).child("✓"))
                    })
                    .when(!item.hint.is_empty(), |row| {
                        row.child(div().text_xs().text_color(rgb(ds.muted)).child(item.hint))
                    })
            }))
            .when(
                menu.group == MenuGroup::File && !self.project.recent.is_empty(),
                |list| {
                    let recent_items = self.project.recent.clone();
                    list.child(
                        div()
                            .mt_1()
                            .pt_1()
                            .border_t_1()
                            .border_color(rgb(ds.frame_border()))
                            .px_1()
                            .text_size(px(9.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(ds.muted))
                            .child("PROYECTOS RECIENTES"),
                    )
                    .children(recent_items.into_iter().enumerate().map(|(i, path)| {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_else(|| path.display().to_string());
                        let folder = path
                            .parent()
                            .and_then(|p| p.file_name())
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        let open_path = path.clone();
                        super::palette::palette_card(ds, ds.cyan, false)
                            .id(("recent-project", i))
                            .flex_shrink_0()
                            .rounded(px(ds.radius_sm))
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_xs()
                                    .text_color(rgb(ds.text))
                                    .child(format!("🕒 {name}")),
                            )
                            .when(!folder.is_empty(), |row| {
                                row.child(
                                    div()
                                        .max_w(px(110.0))
                                        .truncate()
                                        .text_size(px(10.0))
                                        .text_color(rgb(ds.muted))
                                        .child(folder),
                                )
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, _, cx| {
                                    this.bar_menu = None;
                                    this.open_project_path(open_path.clone(), cx);
                                    cx.stop_propagation();
                                    cx.notify();
                                }),
                            )
                    }))
                },
            );
        div()
            .absolute()
            .left_0()
            .right_0()
            .top(top)
            .bottom_0()
            .child(context_menu(
                point(x, px(0.0)),
                on_close,
                menu_panel(ds, width).p_2().child(list),
            ))
    }

    fn arrangement_panel_visible(&self, panel: Panel) -> bool {
        self.center_view == CenterView::Arrangement
            && self
                .workspace_focus
                .expanded
                .is_none_or(|expanded| expanded == panel)
    }

    fn bar_action_checked(&self, action: MenuAction) -> bool {
        match action {
            Command("arrangement") => self.center_view == CenterView::Arrangement,
            Command("pattern") => self.center_view == CenterView::Session,
            Command("mixer") => self.center_view == CenterView::Mixer,
            Command("explorer.toggle") => self.explorer_visible(),
            Command("view.maximize") => self.workspace_focus.expanded.is_some(),
            Fx => self.fx.width > 0.0 && self.arrangement_panel_visible(Panel::Fx),
            PianoRoll => {
                self.piano_roll_window.is_some()
                    || (self.piano_roll.open
                        && self.timeline.midi_clip.is_some()
                        && self.arrangement_panel_visible(Panel::PianoRoll))
            }
            _ => false,
        }
    }

    fn run_bar_action(&mut self, action: MenuAction, window: &mut Window, cx: &mut Context<Self>) {
        self.bar_menu = None;
        let checked = self.bar_action_checked(action);
        match action {
            Command(command) => self.execute_command_with_context(command, window, cx),
            MenuAction::Export => self.submit_command_line("export", window, cx),
            Fx if checked => self.resize_fx(0.0),
            Fx => self.show_fx(),
            PianoRoll if checked => {
                self.execute_toolbar_action(ToolbarAction::RollClose, window, cx)
            }
            PianoRoll => {
                self.set_center_view(CenterView::Arrangement);
                self.workspace_focus.expanded = None;
                if !self.piano_roll.open {
                    self.execute_toolbar_action(ToolbarAction::OpenPianoRoll, window, cx);
                }
            }
        }
        self.sync_windows(cx);
    }

    pub(in crate::ui) fn bar_menu_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(menu) = &mut self.bar_menu else {
            return false;
        };
        let items = menu.group.items();
        match event.keystroke.key.as_str() {
            "escape" => self.bar_menu = None,
            "up" => {
                menu.selected = Some(
                    menu.selected
                        .map_or(items.len() - 1, |i| (i + items.len() - 1) % items.len()),
                )
            }
            "down" => menu.selected = Some(menu.selected.map_or(0, |i| (i + 1) % items.len())),
            "enter" if !event.is_held => {
                let action = items[menu.selected.unwrap_or(0)].action;
                self.run_bar_action(action, window, cx);
            }
            _ => {}
        }
        true
    }
}
