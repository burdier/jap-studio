//! Mesa del proyecto y de Drums Latex. Ambas presentan los canales del audio activo.
mod channel;
mod fader;

use self::channel::{Channel, ChannelSwitches};
use crate::ui::{DawApp, mixer::MixerTarget};
use elevate::musical::{drums::DrumMix, track::UNITY_LEVEL};
use gpui::{Context, Div, Stateful, div, prelude::*, px, rgb};

impl DawApp {
    pub(in crate::ui) fn render_mixer(&self, cx: &mut Context<Self>) -> Stateful<Div> {
        let channels = self
            .tracks
            .iter()
            .enumerate()
            .map(|(index, track)| Channel {
                target: MixerTarget::Track(track.id),
                name: self.track_title(index),
                color: self.ds.track_color(index),
                selected: self.track_selected(index),
                level: self.track_levels[index] as f32 / 100.0,
                unity: UNITY_LEVEL as f32 / 100.0,
                pan: Some(self.track_pans[index]),
                switches: ChannelSwitches::Mix {
                    muted: track.muted,
                    solo: track.solo,
                },
                peak: Some(
                    self.player
                        .as_ref()
                        .map_or(0.0, |player| player.track_peak(track.id)),
                ),
            });
        let utility = self.utility.state();
        self.render_mix_board(
            "project-mixer",
            channels,
            Channel {
                target: MixerTarget::Master,
                name: "Master".into(),
                color: self.ds.cyan,
                selected: false,
                level: utility.gain / 2.0,
                unity: 0.5,
                pan: None,
                switches: ChannelSwitches::Utility {
                    bypassed: utility.bypassed,
                },
                peak: None,
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
        level: mix.fader.level as f32 / 100.0,
        unity: UNITY_LEVEL as f32 / 100.0,
        pan: Some(mix.fader.pan),
        switches: ChannelSwitches::Mix {
            muted: mix.muted,
            solo: mix.solo,
        },
        peak: Some(peak),
    }
}
