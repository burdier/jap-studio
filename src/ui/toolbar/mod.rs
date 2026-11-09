//! Acciones vinculables a iconos. El catálogo headless se consulta sin duplicarlo.
mod catalog;
mod dispatch;
mod picker;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum ToolbarScope {
    Transport,
    PianoRoll,
}

impl ToolbarScope {
    pub fn label(self) -> &'static str {
        match self {
            Self::Transport => "Transporte (Área Libre)",
            Self::PianoRoll => "Piano Roll (Área Libre)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::ui) enum ToolbarAction {
    Catalog(&'static str),
    InsertMidiClip,
    CaptureMidi,
    DuplicateClip,
    ToggleClipMute,
    MidiLearn,
    MidiScan,
    ToggleBypass,
    OpenSettings,
    Record,
    GoToStart,
    ToggleController,
    OpenPalette,
    OpenInstrument,
    OpenPianoRoll,
    DetachInstrument,
    DockInstrument,
    ShowShortcuts,
    TrackMute,
    TrackSolo,
    TrackRecord,
    GainUp,
    GainDown,
    TimelineZoomIn,
    TimelineZoomOut,
    PresetNext,
    PresetPrevious,
    BankNext,
    BankPrevious,
    RollSelect,
    RollDraw,
    RollErase,
    RollPaint,
    RollMute,
    RollGhostNotes,
    RollFollow,
    RollVelocities,
    RollHighlightScale,
    RollToggleWindow,
    RollResetView,
    RollClose,
    RollZoomIn,
    RollZoomOut,
    RollTaller,
    RollShorter,
}
