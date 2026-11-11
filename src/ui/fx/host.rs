//! Conecta eventos del TreeView con catálogo, comandos y pista por identidad estable.
use super::{FxEvent, FxTarget, FxTree};
use crate::ui::{CenterView, DawApp, components::tree::TreeStyle};
use elevate::musical::{fx::FxEdit, track::TrackId};
use gpui::{AppContext, Context, Entity, Subscription, Window};

pub(in crate::ui) struct FxDock {
    pub view: Entity<FxTree>,
    pub analyzers: super::analyzers::Analyzers,
    pub width: f32,
    last_width: f32,
    focus_request: bool,
    track_ids: Vec<TrackId>,
    pub current: Option<FxTarget>,
    pub editor_track: Option<TrackId>,
    pub menu: Option<super::menu::FxMenu>,
    pending: Option<(FxTarget, &'static str)>,
    pub form_target: Option<FxTarget>,
    select_request: Option<FxTarget>,
    instrument_select_request: Option<TrackId>,
    _subscription: Subscription,
}
impl FxDock {
    pub fn new(style: TreeStyle, cx: &mut Context<DawApp>) -> Self {
        let view = cx.new(|cx| FxTree::new(style, cx));
        let subscription = cx.subscribe(&view, |this, _, event, cx| {
            match event {
                FxEvent::Focused => {
                    this.track_delete_pending = false;
                    this.sync_midi_focus();
                }
                FxEvent::Close => this.fx.width = 0.0,
                FxEvent::Selection(target) => {
                    this.fx.current = Some(*target);
                    this.workspace_focus.active = crate::ui::workspace_focus::Panel::Fx;
                },
                FxEvent::Instrument(track) => this.open_track_instrument(*track),
                FxEvent::Add(target) => this.open_fx_palette(*target),
                FxEvent::Action(target, command) => this.fx.pending = Some((*target, *command)),
                FxEvent::Edit(track, edit) => {
                    if let Err(error) = this.edit_fx(*track, edit.clone()) {
                        this.status_message = error;
                    }
                }
                FxEvent::Menu(target, position) => {
                    this.fx.menu = Some(super::menu::FxMenu {
                        target: *target,
                        position: *position,
                        selected: 0,
                    })
                }
            }
            cx.notify();
        });
        Self {
            view,
            analyzers: Default::default(),
            width: 0.0,
            last_width: 300.0,
            focus_request: false,
            track_ids: Vec::new(),
            current: None,
            editor_track: None,
            menu: None,
            pending: None,
            form_target: None,
            select_request: None,
            instrument_select_request: None,
            _subscription: subscription,
        }
    }
}
impl DawApp {
    pub(in crate::ui) fn open_selected_instrument(&mut self) {
        if let Some(track) = self
            .tracks
            .get(self.cursor_row)
            .filter(|track| track.instrument.is_some())
        {
            self.open_track_instrument(track.id);
        } else {
            self.show_fx();
            self.status_message = "Selecciona una pista con instrumento en Track FX".into();
        }
    }
    pub(in crate::ui) fn open_track_instrument(&mut self, track: TrackId) {
        let Some(index) = self
            .tracks
            .iter()
            .position(|item| item.id == track && item.instrument.is_some())
        else {
            return;
        };
        self.select_track(index, false, false);
        if self.vst_selected() {
            self.fx.editor_track = None;
            self.focus_midi_instrument();
            self.fx.instrument_select_request = Some(track);
            self.show_fx();
            self.fx.focus_request = false;
            self.vst.open_editor();
            return;
        }
        self.fx.editor_track = Some(track);
        self.workspace_focus.active = crate::ui::workspace_focus::Panel::Instrument;
        self.focus_midi_instrument();
        self.fx.instrument_select_request = Some(track);
        self.show_fx();
        self.fx.focus_request = false;
    }
    pub(in crate::ui) fn resize_fx(&mut self, width: f32) {
        let width = width.clamp(0.0, 600.0);
        if width >= 40.0 {
            self.fx.last_width = width;
        }
        self.fx.width = if width < 40.0 { 0.0 } else { width };
    }
    pub(in crate::ui) fn show_fx(&mut self) {
        if self.workspace_focus.expanded != Some(crate::ui::workspace_focus::Panel::Fx) {
            self.workspace_focus.expanded = None;
        }
        self.set_center_view(CenterView::Arrangement);
        self.fx.focus_request = true;
        if self.fx.width == 0.0 {
            self.fx.width = self.fx.last_width;
        }
    }
    pub(in crate::ui) fn open_fx_palette(&mut self, target: FxTarget) {
        let Some(index) = self
            .tracks
            .iter()
            .position(|track| track.id == target.track)
        else {
            return;
        };
        self.open_palette_for_track(index);
        self.palette.fx_target = Some(target);
        self.palette.set_category(None);
        self.fx.current = Some(target);
        self.show_fx();
    }
    pub(in crate::ui) fn sync_fx(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some((target, command)) = self.fx.pending.take() {
            self.open_fx_form(target, command, window, cx);
        }
        if self.fx.width == 0.0 && self.fx.view.read(cx).focus.is_focused(window) {
            window.focus(&self.focus_handle, cx);
        }
        if std::mem::take(&mut self.fx.focus_request) && self.action_form.is_none() {
            window.focus(&self.fx.view.read(cx).focus.clone(), cx);
        }
        let style = TreeStyle::from(&self.ds);
        let suspended = self.palette.is_open()
            || self.action_form.is_some()
            || self.fx.menu.is_some()
            || self.settings.is_visible()
            || self.toolbar_icons.picker_open
            || self.track_menu.is_some()
            || self.clip_menu.is_some();
        let tracks_changed = self.fx.track_ids.len() != self.tracks.len()
            || self
                .fx
                .track_ids
                .iter()
                .zip(&self.tracks)
                .any(|(id, track)| *id != track.id);
        if tracks_changed {
            self.fx.track_ids = self.tracks.iter().map(|track| track.id).collect();
        }
        let track = self.tracks.get(self.cursor_row);
        let select = self.fx.select_request.take();
        let instrument_select = self.fx.instrument_select_request.take();
        self.fx.view.update(cx, |tree, cx| {
            tree.vim_enabled = self.vim_enabled;
            if tree.style != style || tree.suspended != suspended {
                tree.style = style;
                tree.suspended = suspended;
                cx.notify();
            }
            if tracks_changed {
                tree.views.retain(|id, _| self.fx.track_ids.contains(id));
            }
            if let Some(track) = track {
                let instrument_name = match track.instrument {
                    Some(id) if id == crate::instruments::vst::VST_ID => self.vst.name.as_str(),
                    Some(id) if id == crate::instruments::sampler::SAMPLER_ID => "Hay Sampler",
                    _ => "Piano SoundFont",
                };
                tree.sync(
                    track.id,
                    &track.name,
                    &track.chain,
                    track.instrument,
                    instrument_name,
                    cx,
                );
            } else if tree.track.take().is_some() {
                tree.rows.clear();
                cx.notify();
            }
            if let Some(id) = instrument_select {
                if let Some(view) = tree.views.get_mut(&id) {
                    view.selected = None;
                    view.instrument_selected = true;
                    view.root_closed = false;
                }
                if tree.track == Some(id) {
                    tree.rebuild();
                    cx.notify();
                }
            }
            if let Some(target) = select {
                if let Some(view) = tree.views.get_mut(&target.track) {
                    view.selected = target.node;
                }
                if tree.track == Some(target.track) {
                    // Mostrar siempre el resultado de Add/duplicar/mover, incluso en un padre cerrado.
                    let mut reveal = false;
                    if let Some(view) = tree.views.get_mut(&target.track) {
                        reveal = view.root_closed;
                        let closed = view.collapsed.len();
                        view.root_closed = false;
                        view.collapsed.retain(|id| {
                            !tree.chain.find(*id).is_some_and(|node| {
                                target.node.is_some_and(|selected| {
                                    *id != selected && node.find(selected).is_some()
                                })
                            })
                        });
                        reveal |= closed != view.collapsed.len();
                    }
                    if reveal {
                        tree.rebuild();
                    }
                    if let Some(index) = tree.rows.iter().position(|row| row.id == target.node) {
                        tree.views
                            .get(&target.track)
                            .unwrap()
                            .scroll
                            .scroll_to_item(index, gpui::ScrollStrategy::Nearest);
                    }
                    cx.notify();
                }
            }
        });
        self.fx.current = self.fx.view.read(cx).target();
    }
    pub(in crate::ui) fn edit_fx(&mut self, track: TrackId, edit: FxEdit) -> Result<(), String> {
        let index = self
            .tracks
            .iter()
            .position(|t| t.id == track)
            .ok_or("La pista ya no existe")?;
        let preserve = matches!(
            edit,
            FxEdit::Toggle(_) | FxEdit::Parameter(..) | FxEdit::Rename(..) | FxEdit::Remove(_)
        );
        let mut chain = self.tracks[index].chain.clone();
        let selected = chain.apply(edit)?;
        if crate::audio::effects::needs_rack(&chain) {
            if !self.tracks[index].accepts_clips() {
                return Err("Spectrum/VST3 requieren una pista de audio o MIDI; los buses de grupo aún no están implementados".into());
            }
            let rate = self
                .vst_effects
                .instances
                .iter()
                .find(|(id, _)| chain.find(**id).is_some())
                .map_or(0, |(_, e)| e.rate);
            self.fx.analyzers.prepare(&chain);
            crate::audio::effects::compile(
                &chain,
                &self.vst_effects.instances,
                &self.fx.analyzers.taps,
                rate,
            )?;
        } else {
            elevate::devices::fx_gain::compile(&chain)?;
        }
        self.tracks[index].chain = chain;
        if !preserve {
            self.fx.select_request = Some(FxTarget {
                track,
                node: selected,
            });
        }
        if !preserve {
            self.show_fx();
        }
        self.sync_vst_effects();
        self.publish_track_fx();
        self.status_message = format!("FX · {}", self.tracks[index].name);
        Ok(())
    }
    pub(in crate::ui) fn fx_node(&self, target: FxTarget) -> Option<&elevate::musical::fx::FxNode> {
        self.tracks
            .iter()
            .find(|t| t.id == target.track)?
            .chain
            .find(target.node?)
    }
    pub(in crate::ui) fn selected_fx_target(&self) -> Option<FxTarget> {
        let track = self.tracks.get(self.cursor_row)?.id;
        Some(
            self.fx
                .current
                .filter(|t| t.track == track)
                .unwrap_or(FxTarget { track, node: None }),
        )
    }
    pub(in crate::ui) fn insert_fx(&mut self, target: FxTarget, id: &str) -> Result<(), String> {
        let descriptor =
            elevate::devices::find(id).ok_or("Este plugin aún no dispone de procesador")?;
        let track = self
            .tracks
            .iter()
            .find(|t| t.id == target.track)
            .ok_or("La pista ya no existe")?;
        if target.node.is_some_and(|id| track.chain.find(id).is_none()) {
            return Err("El destino FX ya no existe".into());
        }
        let placement = track.chain.insertion_after(target.node);
        let node = elevate::musical::fx::FxNode::new(
            descriptor.name,
            elevate::musical::fx::FxKind::Effect {
                plugin_id: elevate::musical::track::TrackDevice::Hay {
                    catalog_id: id.into(),
                },
                parameters: descriptor
                    .parameters
                    .iter()
                    .map(|p| (p.id.0, p.default))
                    .collect(),
            },
        );
        self.edit_fx(target.track, FxEdit::Insert(node, placement))?;
        self.show_fx();
        Ok(())
    }
    pub(in crate::ui) fn request_fx_action(&mut self, target: FxTarget, command: &'static str) {
        self.fx.pending = Some((target, command));
    }
}
