use super::*;
use crate::musical::{
    fx::{FxEdit, FxKind, FxNode, FxPlacement},
    track::{DEMO_TRACKS, TrackDevice, TrackSnapshot},
};

fn snapshot() -> TrackSnapshot {
    TrackSnapshot::capture(&DEMO_TRACKS, &[100; 8], &[50; 8], 7).unwrap()
}

#[test]
fn lua_uses_stable_track_id_and_rejects_stale_or_unknown_handles() {
    let before = snapshot();
    let source = "return {op='instrument.load', track=tracks[6].id, path='/tmp/Synth.vst3'}";
    let plan = build_action(source, &before).unwrap();
    assert_eq!(plan.origin.revision, 7);
    assert_eq!(
        plan.action,
        VstAction::InstrumentLoad {
            track: TrackId(6),
            path: "/tmp/Synth.vst3".into(),
            class: None,
        }
    );
    assert!(
        build_action(
            "return {op='instrument.load', track=999, path='x'}",
            &before
        )
        .is_err()
    );
    assert!(
        build_action(
            "return {op='effect.load', track=6, path='x', after=123}",
            &before
        )
        .is_err()
    );
}

#[test]
fn lua_configures_only_existing_vst_effects_and_rejects_extra_fields() {
    let mut tracks = DEMO_TRACKS;
    let vst = FxNode::new(
        "External",
        FxKind::Effect {
            plugin_id: TrackDevice::Vst3 {
                class_id: "aabb".into(),
            },
            parameters: Default::default(),
        },
    );
    let id = vst.id;
    tracks[5]
        .chain
        .apply(FxEdit::Insert(vst, FxPlacement::Inside(None)))
        .unwrap();
    let before = TrackSnapshot::capture(&tracks, &[100; 8], &[50; 8], 3).unwrap();
    let source = format!("return {{op='effect.toggle', track=6, fx={}}}", id.0);
    assert_eq!(
        build_action(&source, &before).unwrap().action,
        VstAction::EffectToggle {
            track: TrackId(6),
            fx: id
        }
    );
    assert!(build_action("return {op='track.pan', track=6, value=-10}", &before).is_err());
    assert!(build_action("return {op='effect.toggle', track=6, fx=999}", &before).is_err());
    assert!(
        build_action(
            "return {op='instrument.load', track=6, path='x', secret=true}",
            &before
        )
        .is_err()
    );
}
