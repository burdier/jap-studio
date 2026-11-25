//! Mesa del proyecto y de Drums Latex. Ambas presentan los canales del audio activo.
mod channel;
mod fader;

use self::channel::{Channel, ChannelSend, ChannelSwitches, MasterLufs};
use crate::{
    audio::ChannelMeter,
    ui::{DawApp, mixer::MixerTarget},
};
use elevate::musical::{drums::DrumMix, track::UNITY_LEVEL};
use gpui::{Context, Div, Stateful, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_mixer(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let return_buses: Vec<(elevate::musical::track::TrackId, String, String)> = self
            .tracks
            .iter()
            .filter(|track| track.is_return())
            .map(|track| {
                let full = track.name.to_string();
                let short = full
                    .strip_prefix("Return ")
                    .or_else(|| full.strip_prefix("return "))
                    .unwrap_or(&full)
                    .to_string();
                (track.id, short, full)
            })
            .collect();
        let channels = self
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| Channel {
                target: MixerTarget::Track(track.id),
                name: self.track_title(index),
                color: if track.is_return() {
                    self.ds.cyan
                } else {
                    self.ds.track_color(index)
                },
                selected: self.track_selected(index),
                is_return: track.is_return(),
                sends: if track.is_return() {
                    Vec::new()
                } else {
                    return_buses
                        .iter()
                        .map(|(ret_id, short, full)| ChannelSend {
                            target: *ret_id,
                            label: short.clone(),
                            full_name: full.clone(),
                            amount: track.send_amount(*ret_id),
                        })
                        .collect()
                },
                level: self.track_levels[index] as f32 / 100.0,
                unity: UNITY_LEVEL as f32 / 100.0,
                pan: Some(self.track_pans[index]),
                switches: ChannelSwitches::Mix {
                    muted: track.muted,
                    solo: track.solo,
                },
                meter: Some(
                    self.player
                        .as_ref()
                        .map_or_default(|player| player.track_meter(track.id)),
                ),
                spectrum_fx: crate::ui::fx::analyzers::nodes(&track.chain)
                    .into_iter()
                    .find(|(_, enabled)| *enabled)
                    .map(|(node, _)| node.id),
                master_lufs: None,
            });
        let utility = self.utility.state();
        let master = self
            .player
            .as_ref()
            .map(|player| player.master_telemetry())
            .unwrap_or_default();
        self.render_mix_board(
            "project-mixer",
            channels,
            Channel {
                target: MixerTarget::Master,
                name: "Master".into(),
                color: self.ds.cyan,
                selected: false,
                is_return: false,
                sends: Vec::new(),
                level: utility.gain / 2.0,
                unity: 0.5,
                pan: None,
                switches: ChannelSwitches::Utility {
                    bypassed: utility.bypassed,
                },
                meter: Some(ChannelMeter {
                    peak: master.peak,
                    rms: master.rms,
                    hold: master.true_peak_hold[0].max(master.true_peak_hold[1]),
                }),
                spectrum_fx: None,
                master_lufs: Some(MasterLufs {
                    momentary: master.momentary,
                    short_term: master.short_term,
                    integrated: master.integrated,
                }),
            },
            cx,
        )
    }

    pub(in crate::ui) fn render_drum_mixer(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let kit = &self.pattern_editor.session.kit;
        let colors = [
            self.ds.accent,
            self.ds.orange,
            self.ds.cyan,
            self.ds.warning,
        ];
        let channels = kit.pieces.iter().enumerate().map(|(index, piece)| {
            drum_channel(
                MixerTarget::Piece(piece.id),
                piece.name.clone(),
                colors[index % colors.len()],
                index == self.pattern_editor.cursor_lane,
                &piece.mix,
                self.player
                    .as_ref()
                    .map_or(0.0, |player| player.drum_piece_peak(piece.id)),
            )
        });
        self.render_mix_board(
            "kit-mixer",
            channels,
            drum_channel(
                MixerTarget::Drums,
                "Drums".into(),
                self.ds.cyan,
                false,
                &kit.bus,
                self.player
                    .as_ref()
                    .map_or(0.0, |player| player.drum_bus_peak()),
            ),
            cx,
        )
    }

    fn render_mix_board(
        &self,
        id: &'static str,
        channels: impl Iterator<Item = Channel>,
        bus: Channel,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let ds = &self.ds;
        div()
            .id(id)
            .flex_1()
            .min_h_0()
            .min_w_0()
            .flex()
            .overflow_hidden()
            .bg(rgb(ds.gutter_color()))
            .child(
                div()
                    .id("mixer-strips")
                    .flex()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .overflow_x_scroll()
                    .overflow_y_scroll()
                    .gap(px(2.0))
                    .children(channels.map(|channel| self.render_mixer_channel(channel, cx))),
            )
            .child(
                div()
                    .flex()
                    .h_full()
                    .pl(px(4.0))
                    .flex_shrink_0()
                    .child(self.render_mixer_channel(bus, cx)),
            )
    }
}

fn drum_channel(
    target: MixerTarget,
    name: String,
    color: u32,
    selected: bool,
    mix: &DrumMix,
    peak: f32,
) -> Channel {
    Channel {
        target,
        name,
        color,
        selected,
        is_return: false,
        sends: Vec::new(),
        level: mix.fader.level as f32 / 100.0,
        unity: UNITY_LEVEL as f32 / 100.0,
        pan: Some(mix.fader.pan),
        switches: ChannelSwitches::Mix {
            muted: mix.muted,
            solo: mix.solo,
        },
        meter: Some(ChannelMeter {
            peak: [peak, peak],
            rms: [peak * 0.7, peak * 0.7],
            hold: peak,
        }),
        spectrum_fx: None,
        master_lufs: None,
    }
}
