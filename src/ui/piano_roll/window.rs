//! La ventana presenta el mismo editor y observa el mismo clip que el Arrangement.
use crate::ui::DawApp;
use gpui::{
    App, AppContext, Bounds, Context, Entity, FocusHandle, IntoElement, Render, Subscription,
    TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, px, size,
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
        cx.on_release(move |_, cx| {
            // El cierre del sistema devuelve el editor al panel inferior, con su selección.
            if let Some(app) = weak.upgrade() {
                app.update(cx, |app, cx| {
                    app.piano_roll_window = None;
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.app.update(cx, |app, cx| {
            app.render_piano_roll(&self.focus_handle, cx)
                .into_any_element()
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
                if let Err(error) = handle.update(cx, |panel, window, cx| {
                    window.activate_window();
                    window.focus(&panel.focus_handle, cx);
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
                title: Some("Hay · Piano roll".into()),
                ..Default::default()
            }),
            ..Default::default()
        },
        |window, cx| cx.new(|cx| DetachedPianoRoll::new(panel_app, window, cx)),
    );
    app.update(cx, |app, cx| {
        match opened {
            Ok(handle) => app.piano_roll_window = Some(handle),
            Err(error) => app.status_message = format!("No se pudo separar el piano roll: {error}"),
        }
        cx.notify();
    });
}

fn close(handle: WindowHandle<DetachedPianoRoll>, cx: &mut App) {
    if let Err(error) = handle.update(cx, |_, window, _| window.remove_window()) {
        eprintln!("No se pudo cerrar la ventana del piano roll: {error}");
    }
}
