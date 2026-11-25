//! La ventana presenta el mismo editor y observa el mismo clip que el Arrangement.
use crate::ui::DawApp;
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, IntoElement, Render, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div, prelude::*, px, size,
};

pub(in crate::ui) struct DetachedPianoRoll {
    app: Entity<DawApp>,
    focus_handle: FocusHandle,
    _observe_app: Subscription,
}

impl DetachedPianoRoll {
    fn new(app: Entity<DawApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        let weak = app.downgrade();
        let window_id = window.window_handle().window_id();
        cx.on_release(move |_, cx| {
            // El cierre del sistema devuelve el editor al panel inferior, con su selección.
            if let Some(app) = weak.upgrade() {
                app.update(cx, |app, cx| {
                    app.release_chord_audition();
                    app.piano_roll_window = None;
                    if app
                        .action_form
                        .as_ref()
                        .is_some_and(|form| form.window_id == window_id)
                    {
                        app.action_form = None;
                    }
                    cx.notify();
                });
            }
        })
        .detach();
        Self {
            _observe_app: cx.observe(&app, |_, _, cx| cx.notify()),
            app,
            focus_handle,
        }
    }
}

impl Render for DetachedPianoRoll {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app.update(cx, |app, cx| {
            app.sync_audio_selectors(window, cx);
            let content = app
                .render_piano_roll(&self.focus_handle, cx)
                .into_any_element();
            let form = app
                .action_form
                .as_ref()
                .filter(|form| form.window_id == window.window_handle().window_id())
                .map(|_| {
                    app.render_action_form(window.viewport_size(), cx)
                        .into_any_element()
                });
            let picker = app
                .toolbar_icons
                .picker_open
                .then(|| app.render_icon_picker_overlay(cx).into_any_element());
            let settings = app
                .settings
                .is_visible()
                .then(|| app.render_settings_overlay(window, cx).into_any_element());
            let command_bar = app
                .command_open
                .then(|| app.render_command_bar(cx).into_any_element());
            div()
                .relative()
                .size_full()
                .on_mouse_up(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, _| {
                        this.release_chord_audition();
                    }),
                )
                .on_mouse_up_out(
                    gpui::MouseButton::Left,
                    cx.listener(|this, _, _, _| {
                        this.release_chord_audition();
                    }),
                )
                .child(content)
                .children(picker)
                .children(settings)
                .children(form)
                .children(command_bar)
        })
    }
}

impl DawApp {
    pub(in crate::ui) fn toggle_piano_roll_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.piano_roll_window.take() {
            cx.defer(move |cx| close(handle, cx));
        } else if self.piano_roll.open && self.timeline.midi_clip.is_some() {
            // open_window dibuja inmediatamente: hay que salir del update de DawApp primero.
            let app = cx.entity();
            cx.defer(move |cx| open(app, cx));
        }
    }

    pub(in crate::ui) fn focus_piano_roll(&self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(handle) = self.piano_roll_window {
            cx.defer(move |cx| {
                if let Err(error) = handle.update(cx, |root, window, cx| {
                    window.activate_window();
                    if let Ok(panel) = root.view().clone().downcast::<DetachedPianoRoll>() {
                        let focus = panel.read(cx).focus_handle.clone();
                        window.focus(&focus, cx);
                    }
                }) {
                    eprintln!("No se pudo enfocar el piano roll: {error}");
                }
            });
        } else {
            window.focus(&self.piano_roll_focus, cx);
        }
    }
}

fn open(app: Entity<DawApp>, cx: &mut App) {
    if app.read(cx).piano_roll_window.is_some() || !app.read(cx).piano_roll.open {
        return;
    }
    let bounds = Bounds::centered(None, size(px(1258.0), px(654.0)), cx);
    let panel_app = app.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("Jap Studio · Piano roll".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| {
            let panel = cx.new(|cx| DetachedPianoRoll::new(panel_app, window, cx));
            cx.new(|cx| gpui::base::Root::new(panel, window, cx))
        },
    );
    app.update(cx, |app, cx| {
        match opened {
            Ok(handle) => app.piano_roll_window = Some(handle),
            Err(error) => app.status_message = format!("No se pudo separar el piano roll: {error}"),
        }
        cx.notify();
    });
}

fn close(handle: WindowHandle<gpui::base::Root>, cx: &mut App) {
    if let Err(error) = handle.update(cx, |_, window, _| window.remove_window()) {
        eprintln!("No se pudo cerrar la ventana del piano roll: {error}");
    }
}
