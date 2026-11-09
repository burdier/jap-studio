//! Ventanas separadas para componentes con salida de audio (estilo editor de plugin).
//! La ventana solo presenta y reenvía entrada; el estado y el audio siguen en `DawApp`.
use super::DawApp;
use gpui::{
    App, Bounds, Context, Entity, FocusHandle, IntoElement, KeyDownEvent, KeyUpEvent, MouseButton,
    Render, Subscription, TitlebarOptions, Window, WindowBounds, WindowHandle, WindowOptions, div,
    prelude::*, px, rgb, size,
};

/// Vista raíz de una ventana de instrumento desacoplada.
pub(super) struct DetachedPanel {
    app: Entity<DawApp>,
    focus_handle: FocusHandle,
    _observe_app: Subscription,
}

impl DetachedPanel {
    fn new(app: Entity<DawApp>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);
        // Al cerrar la ventana por cualquier vía, el componente vuelve a acoplarse.
        let weak = app.downgrade();
        cx.on_release(move |_, cx| {
            weak.update(cx, |app, cx| {
                app.instrument_window = None;
                app.all_notes_off();
                cx.notify();
            })
            .ok();
        })
        .detach();
        Self {
            _observe_app: cx.observe(&app, |_, _, cx| cx.notify()),
            app,
            focus_handle,
        }
    }
}

impl Render for DetachedPanel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (content, bg, text) = self.app.update(cx, |app, cx| {
            (
                app.render_instrument(cx).into_any_element(),
                app.ds.bg,
                app.ds.text,
            )
        });
        div()
            .id("hay-detached-instrument")
            .track_focus(&self.focus_handle)
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(bg))
            .text_color(rgb(text))
            .font_family("monospace")
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, window, cx| {
                    window.focus(&this.focus_handle, cx);
                    this.app.update(cx, |app, cx| {
                        app.focus_midi_instrument();
                        cx.notify();
                    });
                }),
            )
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                this.app.update(cx, |app, cx| {
                    app.focus_midi_instrument();
                    app.handle_detached_key(event);
                    cx.notify();
                });
            }))
            .on_key_up(cx.listener(|this, event: &KeyUpEvent, _, cx| {
                this.app.update(cx, |app, cx| {
                    app.handle_key_up(event);
                    cx.notify();
                });
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |app, cx| {
                        app.release_mouse_note();
                        cx.notify();
                    });
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.app.update(cx, |app, cx| {
                        app.release_mouse_note();
                        cx.notify();
                    });
                }),
            )
            .child(content)
    }
}

impl DawApp {
    pub(super) fn instrument_detached(&self) -> bool {
        self.instrument_window.is_some()
    }

    /// La ventana separada comparte transporte; Esc conserva la liberación de notas.
    pub(super) fn handle_detached_key(&mut self, event: &KeyDownEvent) {
        if self.handle_transport_key(event) {
            return;
        }
        let key = event
            .keystroke
            .key_char
            .as_deref()
            .unwrap_or(&event.keystroke.key);
        if self.controller_key_down(key, event.is_held) {
            return;
        }
        if key == "escape" {
            self.all_notes_off();
        }
    }

    /// Atiende las peticiones de ventana que nacen en el teclado (`:detach`, `:dock`).
    pub(super) fn sync_windows(&mut self, cx: &mut Context<Self>) {
        match self.window_request.take() {
            Some(true) if !self.instrument_detached() => self.toggle_instrument_window(cx),
            Some(false) if self.instrument_detached() => self.toggle_instrument_window(cx),
            _ => {}
        }
    }

    pub(super) fn toggle_instrument_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.instrument_window.take() {
            // Cerrar fuera del evento en curso; on_release ya no encontrará handle.
            cx.defer(move |cx| close(handle, cx));
            self.status_message = "Instrumento acoplado".into();
            return;
        }
        // `open_window` dibuja la ventana nueva al instante y esta lee `DawApp`:
        // debe llamarse fuera de cualquier `update` de `DawApp`.
        let app = cx.entity();
        cx.defer(move |cx| open_instrument_window(app, cx));
    }
}

fn open_instrument_window(app: Entity<DawApp>, cx: &mut App) {
    if app.read(cx).instrument_window.is_some() {
        return;
    }
    let name = app.read(cx).active_instrument_name().to_string();
    let bounds = Bounds::centered(None, size(px(900.0), px(640.0)), cx);
    let panel_app = app.clone();
    let opened = cx.open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(name.clone().into()),
                ..Default::default()
            }),
            app_id: Some("hay-daw".to_string()),
            ..Default::default()
        },
        |window, cx| {
            let panel = cx.new(|cx| DetachedPanel::new(panel_app, window, cx));
            cx.new(|cx| gpui::base::Root::new(panel, window, cx))
        },
    );
    app.update(cx, |this, cx| {
        match opened {
            Ok(handle) => {
                this.instrument_window = Some(handle);
                this.status_message = format!("{name} en ventana separada");
            }
            Err(error) => this.status_message = format!("No se pudo abrir la ventana: {error}"),
        }
        cx.notify();
    });
}

fn close(handle: WindowHandle<gpui::base::Root>, cx: &mut App) {
    handle
        .update(cx, |_, window, _| window.remove_window())
        .ok();
}
