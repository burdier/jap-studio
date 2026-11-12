//! Guardado versionado de asignaciones de iconos en el perfil existente.
use super::{ToolbarAction, ToolbarScope};
use crate::ui::{
    icons::{BoundToolButton, ToolbarBindings},
    profile,
};
use std::io;

impl ToolbarBindings {
    pub(in crate::ui) fn save_bindings(&self) -> io::Result<()> {
        profile::save_values(&[
            (
                "TOOLBAR_TRANSPORT",
                encode_bindings(&self.transport_buttons),
            ),
            (
                "TOOLBAR_PIANO_ROLL",
                encode_bindings(&self.piano_roll_buttons),
            ),
        ])
    }
}

pub(in crate::ui) fn restore_bindings(bindings: &mut ToolbarBindings, profile: &str) {
    if let Some(value) = profile_value(profile, "TOOLBAR_TRANSPORT")
        && let Some(buttons) = decode_bindings(value, ToolbarScope::Transport)
    {
        bindings.transport_buttons = buttons;
    }
    if let Some(value) = profile_value(profile, "TOOLBAR_PIANO_ROLL")
        && let Some(buttons) = decode_bindings(value, ToolbarScope::PianoRoll)
    {
        bindings.piano_roll_buttons = buttons;
    }
}

fn profile_value<'a>(profile: &'a str, key: &str) -> Option<&'a str> {
    profile
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter_map(|(name, value)| (name.trim() == key).then_some(value.trim()))
        .last()
}

fn encode_bindings(buttons: &[BoundToolButton]) -> String {
    let mut encoded = String::from("v1");
    for button in buttons {
        let Some(action_key) = button.action.profile_key() else {
            continue;
        };
        encoded.push(';');
        encoded.push_str(&encode_hex(button.icon_id.as_bytes()));
        encoded.push(':');
        encoded.push_str(&encode_hex(action_key.as_bytes()));
        encoded.push(':');
        match &button.custom_command {
            Some(command) => {
                encoded.push('1');
                encoded.push_str(&encode_hex(command.as_bytes()));
            }
            None => encoded.push('0'),
        }
    }
    encoded
}

fn decode_bindings(encoded: &str, scope: ToolbarScope) -> Option<Vec<BoundToolButton>> {
    let mut records = encoded.split(';');
    if records.next()? != "v1" {
        return None;
    }
    let mut buttons: Vec<BoundToolButton> = Vec::new();
    for record in records {
        let mut fields = record.split(':');
        let (Some(icon), Some(action), Some(command), None) =
            (fields.next(), fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        let Some(icon_id) = decode_hex(icon).and_then(|bytes| String::from_utf8(bytes).ok()) else {
            continue;
        };
        if icon_id.is_empty() {
            continue;
        }
        let action_key = decode_hex(action).and_then(|bytes| String::from_utf8(bytes).ok());
        let custom_command = if command == "0" {
            None
        } else if let Some(command) = command.strip_prefix('1') {
            let Some(command) = decode_hex(command).and_then(|bytes| String::from_utf8(bytes).ok())
            else {
                continue;
            };
            let trimmed = command.trim().trim_start_matches(':').trim();
            if trimmed.is_empty() || trimmed.len() > 256 || trimmed.contains(['\n', '\r', '\0']) {
                continue;
            }
            Some(trimmed.to_owned())
        } else {
            continue;
        };
        let action = action_key
            .as_deref()
            .and_then(|key| ToolbarAction::from_profile_key(scope, key))
            .or_else(|| custom_command.as_ref().map(|_| default_action(scope)));
        let Some(action) = action else {
            continue;
        };
        if (custom_command.is_none()
            && buttons
                .iter()
                .any(|button| button.custom_command.is_none() && button.action == action))
            || custom_command.as_ref().is_some_and(|command| {
                buttons
                    .iter()
                    .any(|button| button.custom_command.as_ref() == Some(command))
            })
        {
            continue;
        }
        buttons.push(BoundToolButton {
            icon_id,
            action,
            custom_command,
        });
    }
    Some(buttons)
}

fn default_action(scope: ToolbarScope) -> ToolbarAction {
    match scope {
        ToolbarScope::Transport => ToolbarAction::InsertMidiClip,
        ToolbarScope::PianoRoll => ToolbarAction::RollGhostNotes,
    }
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(HEX[usize::from(*byte >> 4)] as char);
        encoded.push(HEX[usize::from(*byte & 0x0f)] as char);
    }
    encoded
}

fn decode_hex(encoded: &str) -> Option<Vec<u8>> {
    fn nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }
    if encoded.len() % 2 != 0 {
        return None;
    }
    encoded
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}
