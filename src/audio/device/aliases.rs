//! Agrupa rutas ALSA del mismo endpoint físico; nunca agrupa por nombre solamente.
//! /proc se consulta exclusivamente en el worker de inventario.
use super::inventory::DeviceInfo;
use std::collections::HashMap;

pub(super) fn consolidate(devices: Vec<DeviceInfo>) -> Vec<DeviceInfo> {
    if !cfg!(target_os = "linux") {
        return devices;
    }
    let cards = card_ids();
    let hardware: HashMap<_, _> = devices
        .iter()
        .filter(|d| d.id.starts_with("alsa:hw:"))
        .filter_map(|d| endpoint(&d.id, &cards).map(|key| (key, (d.id.clone(), d.name.clone()))))
        .collect();
    let mut aliases: HashMap<String, Vec<String>> = HashMap::new();
    let mut result = Vec::new();
    for device in devices {
        let physical = endpoint(&device.id, &cards).and_then(|key| hardware.get(&key));
        if let Some((id, _)) = physical.filter(|(id, name)| {
            *id != device.id
                && (*name == device.name
                    || device.id.starts_with("alsa:plughw:")
                    || (device.id.starts_with("alsa:sysdefault:")
                        && endpoint(&device.id, &cards).is_some_and(|(card, _)| {
                            hardware
                                .keys()
                                .filter(|(candidate, _)| candidate == &card)
                                .count()
                                == 1
                        })))
        }) {
            aliases.entry(id.clone()).or_default().push(device.id);
        } else {
            result.push(device);
        }
    }
    for device in &mut result {
        device.aliases = aliases.remove(&device.id).unwrap_or_default();
    }
    result
}

fn card_ids() -> HashMap<String, String> {
    let mut cards = HashMap::new();
    let Ok(entries) = std::fs::read_dir("/proc/asound") else {
        return cards;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(index) = name.to_str().and_then(|s| s.strip_prefix("card")) else {
            continue;
        };
        if index.parse::<u32>().is_err() {
            continue;
        }
        if let Ok(id) = std::fs::read_to_string(entry.path().join("id")) {
            cards.insert(id.trim().to_owned(), index.to_owned());
        }
    }
    cards
}

fn endpoint(id: &str, cards: &HashMap<String, String>) -> Option<(String, u32)> {
    let (route, parameters) = id.strip_prefix("alsa:")?.split_once(':')?;
    // Conserva rutas personalizadas, servidores y dispositivos sin identidad verificable.
    if !matches!(
        route,
        "hw" | "plughw"
            | "sysdefault"
            | "front"
            | "surround21"
            | "surround40"
            | "surround41"
            | "surround50"
            | "surround51"
            | "surround71"
            | "iec958"
    ) {
        return None;
    }
    let mut card = None;
    let mut device = None;
    for part in parameters.split(',') {
        let (key, value) = part.split_once('=')?;
        match key {
            "CARD" => card = Some(value),
            "DEV" => device = Some(value.parse::<u32>().ok()?),
            _ => return None,
        }
    }
    let card = card?;
    let index = match card.parse::<u32>() {
        Ok(index) => index.to_string(),
        Err(_) => cards.get(card)?.clone(),
    };
    let device = device.or_else(|| (route == "sysdefault").then_some(0))?;
    Some((index, device))
}
