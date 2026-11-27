//! Ventana original devuelta por createView("editor"), sin reconstruir controles.
#[cfg(target_os = "linux")]
mod platform {
    use super::super::{
        EditorHandle, EditorMidiKey,
        frame::{Frame, valid},
        shortcut::SpaceShortcut,
        x11::X11,
    };
    use std::{ptr, rc::Rc, sync::atomic::Ordering};
    use vst3::{
        ComPtr, ComWrapper,
        Steinberg::{Vst::*, *},
    };
    pub(crate) struct Window {
        view: ComPtr<IPlugView>,
        frame: ComWrapper<Frame>,
        _frame_interface: ComPtr<IPlugFrame>,
        x11: Rc<X11>,
        attached: bool,
        space: SpaceShortcut,
        midi_enabled: bool,
        midi_held: [Option<u8>; 256],
    }
    impl Window {
        pub fn open(controller: &ComPtr<IEditController>, name: &str) -> Result<Self, String> {
            unsafe {
                let view = ComPtr::from_raw(controller.createView(c"editor".as_ptr()))
                    .ok_or("Este plugin no ofrece un editor gráfico")?;
                super::super::super::check(
                    view.isPlatformTypeSupported(c"X11EmbedWindowID".as_ptr()),
                    "editor X11/XWayland",
                )?;
            let mut rect = ViewRect {
                left: 0,
                top: 0,
                right: 800,
                bottom: 600,
            };
            super::super::super::check(view.getSize(&mut rect), "tamaño del editor")?;
            let size = (rect.right - rect.left, rect.bottom - rect.top);
            if !valid(size) {
                return Err("El plugin devolvió un tamaño de editor inválido".into());
            }
            let x11 = X11::open(
                &format!("{name} — Jap Studio"),
                size.0 as u32,
                size.1 as u32,
            )?;
            let frame = ComWrapper::new(Frame::new(x11.clone(), size));
            let interface = frame.to_com_ptr::<IPlugFrame>().unwrap();
            let mut window = Self {
                view,
                frame,
                _frame_interface: interface,
                x11,
                attached: false,
                space: SpaceShortcut::default(),
                midi_enabled: false,
                midi_held: [None; 256],
            };
            window.x11.raise();
            super::super::super::check(
                window.view.setFrame(window._frame_interface.as_ptr()),
                "setFrame",
            )?;
            super::super::super::check(
                window
                    .view
                    .attached(window.x11.window as *mut _, c"X11EmbedWindowID".as_ptr()),
                "abrir editor original",
            )?;
            window.attached = true;
            window.x11.raise();
            Ok(window)
        }
    }
    pub fn raise(&self) {
        self.x11.raise();
    }
    fn release_midi(&mut self, handle: &EditorHandle) {
        if self.midi_held.iter().any(Option::is_some) {
            self.midi_held.fill(None);
            handle.push_midi_key(EditorMidiKey::Release);
        }
    }

    /// false solicita cerrar solo la ventana; el instrumento continúa procesando audio.
    pub fn tick(&mut self, handle: &EditorHandle) -> bool {
        let midi = handle.midi_enabled.load(Ordering::Acquire);
        if midi != self.midi_enabled {
            self.midi_enabled = midi;
            self.x11.grab_midi_keys(midi);
            if !midi {
                self.release_midi(handle);
            }
        }
        self.frame.tick();
        for _ in 0..256 {
            let Some(mut event) = self.x11.poll() else {
                break;
            };
            unsafe {
                match event.kind {
                    33 if event.client.window == self.x11.window
                        && event.client.message_type == self.x11.protocols
                        && event.client.format == 32
                        && event.client.data[0] as u64 == self.x11.delete =>
                    {
                        return false;
                    }
                    22 if event.configure.window == self.x11.window => {
                        let size = (event.configure.width, event.configure.height);
                        if valid(size) && size != self.frame.size.get() {
                            if self.view.canResize() == kResultOk {
                                let mut rect = ViewRect {
                                    left: 0,
                                    top: 0,
                                    right: size.0,
                                    bottom: size.1,
                                };
                                self.view.checkSizeConstraint(&mut rect);
                                if valid((rect.right, rect.bottom)) {
                                    self.frame.resizeView(self.view.as_ptr(), &mut rect);
                                }
                            } else {
                                let (w, h) = self.frame.size.get();
                                self.x11.resize(w as u32, h as u32);
                            }
                        }
                    }
                    19 => self.x11.focus(),
                    9 => {
                        self.view.onFocus(1);
                    }
                    10 => {
                        self.view.onFocus(0);
                        if matches!(event.focus.mode, 0 | 3) {
                            self.release_midi(handle);
                        }
                    }
                    2 | 3 => {
                        let down = event.kind == 2;
                        let (character, symbol) = self.x11.key(&mut event.key);
                        let state = event.key.state;
                        if let Some(held) = self.midi_held.get_mut(event.key.keycode as usize) {
                            if !down {
                                if let Some(key) = held.take() {
                                    handle.push_midi_key(EditorMidiKey::Key {
                                        key,
                                        down,
                                        repeat: false,
                                    });
                                    continue;
                                }
                            } else if self.midi_enabled && state & (1 | 4 | 8 | 64) == 0 {
                                let key = (symbol as u8).to_ascii_lowercase();
                                if symbol < 128
                                    && elevate::musical::controller::computer_midi_keys()
                                        .any(|mapped| mapped.as_bytes()[0] == key)
                                {
                                    let repeat = held.replace(key).is_some();
                                    handle.push_midi_key(EditorMidiKey::Key { key, down, repeat });
                                    continue;
                                }
                            }
                        }
                        if symbol == 0x20 {
                            if !down {
                                self.space.key(false, event.key.time as u64);
                                if state & (1 | 4 | 8 | 64) == 0 {
                                    continue;
                                }
                            } else if state & (1 | 4 | 8 | 64) == 0 {
                                if self.space.key(true, event.key.time as u64) {
                                    handle
                                        .transport_toggles
                                        .fetch_add(1, std::sync::atomic::Ordering::Release);
                                }
                                continue;
                            }
                        }
                        let modifiers = ((state & 1 != 0) as i16)
                            | (((state & 8 != 0) as i16) << 1)
                            | (((state & 4 != 0) as i16) << 2);
                        if down {
                            self.view.onKeyDown(character, 0, modifiers);
                        } else {
                            self.view.onKeyUp(character, 0, modifiers);
                        }
                    }
                    _ => {}
                }
            }
        }
        true
    }
}
    impl Drop for Window {
        fn drop(&mut self) {
            unsafe {
                if self.attached {
                    self.view.onFocus(0);
                    self.view.removed();
                }
                self.view.setFrame(ptr::null_mut());
            }
            // El plugin puede retener el frame. La ventana se destruye al cerrar
            // la vista aunque el frame viva más tiempo.
            self.x11.close_window();
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use super::super::EditorHandle;
    use vst3::{ComPtr, Steinberg::Vst::IEditController};

    pub(crate) struct Window;

    impl Window {
        pub fn open(_controller: &ComPtr<IEditController>, _name: &str) -> Result<Self, String> {
            Err("El editor gráfico VST3 nativo aún está en preparación para esta plataforma; el audio y los parámetros siguen activos".into())
        }

        pub fn raise(&self) {}

        pub fn tick(&mut self, _handle: &EditorHandle) -> bool {
            false
        }
    }
}

pub(super) use platform::Window;
