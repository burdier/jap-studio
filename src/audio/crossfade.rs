//! Coeficientes de cruce compartidos por Play, exportación y medición de picos.
use super::AudioClip;
use elevate::musical::clip::ClipId;
use std::collections::HashMap;

#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Ramp {
    Cross(LinkRamp),
    Edge {
        start: f64,
        scale: f64,
        tension: f32,
        outgoing: bool,
    },
}

#[derive(Clone, Copy, PartialEq)]
pub(crate) struct LinkRamp {
    active: (u64, u64),
    first: u64,
    last: u64,
    pivot: u64,
    left_scale: f32,
    right_scale: f32,
    incoming: bool,
    returns: bool,
}

impl Ramp {
    pub fn gain(&self, frame: u64) -> f32 {
        match *self {
            Self::Cross(curve) => curve.gain(frame),
            Self::Edge {
                start,
                scale,
                tension,
                outgoing,
            } => {
                let t = ((frame as f64 - start) * scale) as f32;
                elevate::musical::sample_edit::ClipFade::curve(
                    if outgoing { 1. - t } else { t },
                    tension,
                )
            }
        }
    }
}
impl LinkRamp {
    pub fn gain(&self, frame: u64) -> f32 {
        if frame < self.active.0
            || frame >= self.active.1
            || frame < self.first
            || frame > self.last
        {
            return 1.;
        }
        let x = if self.returns {
            if frame <= self.pivot {
                (frame - self.first) as f32 * self.left_scale
            } else {
                (self.last - frame) as f32 * self.right_scale
            }
        } else if frame <= self.pivot {
            0.5 * (frame - self.first) as f32 * self.left_scale
        } else {
            1. - 0.5 * (self.last - frame) as f32 * self.right_scale
        };
        if self.incoming { x } else { 1. - x }
    }
}

pub(crate) fn prepare<'a>(
    clips: impl Iterator<Item = &'a AudioClip> + Clone,
    rate: u32,
    bpm: f64,
) -> HashMap<ClipId, Vec<Ramp>> {
    let mut result = HashMap::<ClipId, Vec<Ramp>>::new();
    let rate_f = f64::from(rate);
    for clip in clips.clone().filter(|clip| !clip.props.muted) {
        let origin =
            (clip.start_beat * 60. / bpm * rate_f).round() - (clip.trim_start * rate_f).round();
        for (side, fade) in clip
            .processing
            .edit
            .fades
            .into_iter()
            .enumerate()
            .filter(|(_, f)| f.active())
        {
            let first = (fade.start_us as f64 * rate_f / 1e6).round();
            let last = (fade.end_us as f64 * rate_f / 1e6).round() - 1.;
            result.entry(clip.id).or_default().push(Ramp::Edge {
                start: origin + first,
                scale: 1. / (last - first).max(1.),
                tension: fade.tension(),
                outgoing: side == 1,
            });
        }
    }
    if !clips.clone().any(|c| c.processing.edit.crossfade.is_some()) {
        return result;
    }
    let span = |c: &AudioClip| {
        let start = (c.start_beat / (bpm / 60.) * f64::from(rate)).round() as u64;
        (
            start,
            start.saturating_add((c.length_seconds() * f64::from(rate)).round() as u64),
        )
    };
    let mut families = HashMap::<_, Vec<&AudioClip>>::new();
    for c in clips.clone().filter(|c| !c.props.muted) {
        families
            .entry((c.track, c.crossfade_root()))
            .or_default()
            .push(c);
    }
    // Solo unen la curva los fragmentos que aún son contiguos tanto en el arreglo como en la fuente.
    let mut extents = HashMap::new();
    for pieces in families.values_mut() {
        pieces.sort_unstable_by_key(|c| (span(c).0, c.id.value()));
        let mut run = 0;
        for end in 1..=pieces.len() {
            if let Some(next) = pieces.get(end) {
                let prev = pieces[end - 1];
                if span(prev).1.abs_diff(span(next).0) <= 1
                    && ((prev.trim_start + prev.length_seconds() - next.trim_start)
                        * f64::from(rate))
                    .abs()
                        <= 1.
                {
                    continue;
                }
            }
            let extent = (span(pieces[run]).0, span(pieces[end - 1]).1);
            for c in &pieces[run..end] {
                extents.insert(c.id, extent);
            }
            run = end;
        }
    }
    for to in clips.filter(|c| !c.props.muted) {
        let Some(link) = to.processing.edit.crossfade else {
            continue;
        };
        let Some(sources) = families.get(&(to.track, link.from)) else {
            continue;
        };
        let b = extents[&to.id];
        let t = span(to);
        for from in sources {
            if from.id == to.id {
                continue;
            }
            let f = span(from);
            let active = (f.0.max(t.0), f.1.min(t.1));
            if active.0 >= active.1 {
                continue;
            }
            let a = extents[&from.id];
            let first = a.0.max(b.0);
            let Some(last) = a.1.min(b.1).checked_sub(1) else {
                continue;
            };
            if last.saturating_sub(first) < 2 {
                continue;
            }
            let pivot = first
                + (((last - first) as f64 * f64::from(link.center) / 100.).round() as u64)
                    .clamp(1, last - first - 1);
            let reverse = a > b;
            let returns = if reverse { b.1 > a.1 } else { a.1 > b.1 };
            let ramp = LinkRamp {
                active,
                first,
                last,
                pivot,
                left_scale: 1. / (pivot - first) as f32,
                right_scale: 1. / (last - pivot) as f32,
                incoming: reverse,
                returns,
            };
            result.entry(from.id).or_default().push(Ramp::Cross(ramp));
            result.entry(to.id).or_default().push(Ramp::Cross(LinkRamp {
                incoming: !reverse,
                ..ramp
            }));
        }
    }
    result
}
