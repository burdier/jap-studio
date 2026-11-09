//! Inicialización de GPUI Kit y proyección del tema de Hay, sin estado de audio.
use super::{DawApp, DesignSystem};
use gpui::{
    App, Context, Global, KeyBinding, KeyDownEvent, Keystroke, Modifiers, Window,
    component::{Theme, ThemeMode},
    px, rgb,
};

gpui::actions!(
    hay_ui,
    [
        WorkspaceTab,
        WorkspaceBackTab,
        ConfirmActionForm,
        CancelActionForm
    ]
);

pub(super) fn init(cx: &mut App) {
    gpui::init(cx);
    gpui::component::set_locale("es");
    cx.bind_keys([
        KeyBinding::new("tab", WorkspaceTab, Some("HayWorkspace")),
        KeyBinding::new("shift-tab", WorkspaceBackTab, Some("HayWorkspace")),
        KeyBinding::new("escape", CancelActionForm, Some("HayActionForm")),
        KeyBinding::new("enter", ConfirmActionForm, Some("HayActionForm")),
        KeyBinding::new("ctrl-enter", ConfirmActionForm, Some("HayActionForm")),
    ]);
}

impl DawApp {
    /// El Root de Kit gestiona Tab dentro del formulario. El workspace mantiene sus atajos.
    pub(in crate::ui) fn handle_workspace_tab(
        &mut self,
        backwards: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.palette.is_open()
            && !self.settings.is_visible()
            && !self.toolbar_icons.picker_open
            && (self.explorer.view.read(cx).focus.is_focused(window)
                || self.piano_roll_focus.is_focused(window)
                || self.fx.view.read(cx).focus.is_focused(window))
        {
            return;
        }
        let event = KeyDownEvent {
            keystroke: Keystroke {
                modifiers: Modifiers {
                    shift: backwards,
                    ..Default::default()
                },
                key: "tab".into(),
                key_char: None,
            },
            is_held: false,
            prefer_character_input: false,
        };
        self.handle_key(&event, window, cx);
        self.sync_windows(cx);
        cx.notify();
    }
}

#[derive(Clone, PartialEq)]
struct AppliedTheme {
    colors: [u32; 9],
    radius: f32,
    large_radius: f32,
    font_size: f32,
}
impl Global for AppliedTheme {}

pub(super) fn sync_theme(ds: &DesignSystem, cx: &mut App) {
    let signature = AppliedTheme {
        colors: [
            ds.bg,
            ds.panel,
            ds.panel_dark,
            ds.border,
            ds.text,
            ds.muted,
            ds.accent,
            ds.pink,
            ds.selection_background(),
        ],
        radius: ds.radius_sm,
        large_radius: ds.radius_lg,
        font_size: ds.font_size_xs + 2.0,
    };
    if cx.try_global::<AppliedTheme>() == Some(&signature) {
        return;
    }
    let background: gpui::Hsla = rgb(ds.bg).into();
    let mode = if background.l < 0.5 {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    if Theme::global(cx).mode != mode {
        Theme::change(mode, None, cx);
    }
    Theme::update(cx, |theme| {
        let colors = &mut theme.colors;
        colors.background = rgb(ds.bg).into();
        colors.foreground = rgb(ds.text).into();
        colors.border = rgb(ds.border).into();
        colors.input = rgb(ds.border).into();
        colors.caret = rgb(ds.accent).into();
        colors.ring = rgb(ds.accent).into();
        colors.selection = rgb(ds.selection_background()).into();
        colors.primary = rgb(ds.accent).into();
        colors.primary_foreground = rgb(ds.bg).into();
        colors.secondary = rgb(ds.panel).into();
        colors.secondary_foreground = rgb(ds.text).into();
        colors.muted = rgb(ds.panel_dark).into();
        colors.muted_foreground = rgb(ds.muted).into();
        colors.accent = rgb(ds.selection_background()).into();
        colors.accent_foreground = rgb(ds.text).into();
        colors.popover = rgb(ds.panel_dark).into();
        colors.popover_foreground = rgb(ds.text).into();
        colors.danger = rgb(ds.pink).into();
        colors.button = rgb(ds.panel).into();
        colors.button_foreground = rgb(ds.text).into();
        colors.button_hover = rgb(ds.selection_background()).into();
        colors.button_active = rgb(ds.selection_background()).into();
        colors.button_primary = rgb(ds.accent).into();
        colors.button_primary_foreground = rgb(ds.bg).into();
        colors.button_primary_hover = rgb(ds.accent).into();
        colors.button_primary_active = rgb(ds.accent).into();
        theme.radius = px(ds.radius_sm);
        theme.radius_lg = px(ds.radius_lg);
        theme.font_family = "monospace".into();
        theme.font_size = px(ds.font_size_xs + 2.0);
        theme.shadow = false;
        theme.focus_ring = false;
    });
    cx.set_global(signature);
}
