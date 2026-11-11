//! Datos de presentación de audio, independientes de las entidades Kit.
use super::{AudioChange as Change, AudioField as Field};
use crate::ui::DawApp;
pub(super) type Choices = Vec<(String, Change, bool)>;
pub(super) type SelectorModel = (Field, &'static str, String, Choices);
impl DawApp {
    pub(super) fn audio_selector_models(&self) -> Vec<SelectorModel> {
        let audio = &self.settings.audio;
        let draft = &audio.draft;
        let inventory = &audio.inventory;
        let output = inventory.selected(draft, false);
        let input = inventory.selected(draft, true);
        let device_name = |input: bool| {
            let selected = if input { &draft.input } else { &draft.output };
            match (selected, inventory.selected(draft, input)) {
                (None, Some(d)) => format!("Predeterminado · {}", d.name),
                (None, None) => "Predeterminado del sistema".into(),
                (Some(_), Some(d)) => d.name.clone(),
                (Some(id), None) => format!("No disponible · {id}"),
            }
        };
        let mut drivers = vec![(
            "Predeterminado".into(),
            Change::Driver(None),
            draft.host.is_none(),
        )];
        drivers.extend(inventory.hosts.iter().map(|name| {
            (
                name.clone(),
                Change::Driver(Some(name.clone())),
                draft.host.as_ref() == Some(name),
            )
        }));
        let devices = |input: bool| -> Choices {
            let selected = if input { &draft.input } else { &draft.output };
            let change = |id| {
                if input {
                    Change::Input(id)
                } else {
                    Change::Output(id)
                }
            };
            let mut rows = vec![(
                "Predeterminado del sistema".into(),
                change(None),
                selected.is_none(),
            )];
            rows.extend(
                inventory
                    .devices
                    .iter()
                    .enumerate()
                    .filter(|(_, d)| d.supports(input))
                    .map(|(index, d)| {
                        (
                            format!(
                                "{} · {} · {}",
                                index + 1,
                                d.name,
                                if d.error(input).is_some() {
                                    "Consultar estado".into()
                                } else {
                                    d.status(input)
                                }
                            ),
                            change(Some(d.id.clone())),
                            selected.as_ref().is_some_and(|id| d.matches_id(id)),
                        )
                    }),
            );
            rows
        };
        let mut rates = vec![(
            "Auto · frecuencia del dispositivo".into(),
            Change::Rate(None),
            draft.sample_rate.is_none(),
        )];
        if let Some(output) = output {
            rates.extend(output.rates(false).into_iter().map(|rate| {
                (
                    format!("{rate} Hz"),
                    Change::Rate(Some(rate)),
                    draft.sample_rate == Some(rate),
                )
            }));
        }
        let mut buffers = vec![(
            "Auto · gestionado por el driver".into(),
            Change::Buffer(None),
            draft.buffer_frames.is_none(),
        )];
        if let Some(output) = output {
            buffers.extend(
                output
                    .buffers(draft.sample_rate, draft.output_pair)
                    .into_iter()
                    .map(|n| {
                        (
                            format!("{n} samples"),
                            Change::Buffer(Some(n)),
                            draft.buffer_frames == Some(n),
                        )
                    }),
            );
        }
        let outputs = if let Some(output) = output.filter(|d| d.shared()) {
            inventory
                .devices
                .iter()
                .filter(|d| d.supports(false) && (d.id == output.id || d.same_output_group(output)))
                .flat_map(|d| {
                    d.output_pairs().map(move |n| {
                        (
                            d.output_label(n),
                            Change::OutputRoute(d.id.clone(), n),
                            d.id == output.id && draft.output_pair == n,
                        )
                    })
                })
                .collect()
        } else {
            output
                .into_iter()
                .flat_map(|d| {
                    d.output_pairs().map(move |n| {
                        (
                            d.output_label(n),
                            Change::OutputPair(n),
                            draft.output_pair == n,
                        )
                    })
                })
                .collect()
        };
        let inputs = (0..input.map_or(0, |d| d.channels(true).min(256)))
            .map(|n| {
                (
                    format!("{} · mono", n + 1),
                    Change::InputChannel(n),
                    draft.input_channel == n,
                )
            })
            .collect();
        vec![
            (
                Field::Driver,
                "Tipo de driver",
                draft
                    .host
                    .clone()
                    .unwrap_or_else(|| format!("Auto · {}", inventory.host)),
                drivers,
            ),
            (
                Field::Output,
                "Dispositivo de salida",
                device_name(false),
                devices(false),
            ),
            (
                Field::OutputPair,
                "Salida estéreo",
                output.map_or_else(
                    || "Selecciona una salida disponible".into(),
                    |d| d.output_label(draft.output_pair),
                ),
                outputs,
            ),
            (
                Field::Input,
                "Dispositivo de entrada",
                device_name(true),
                devices(true),
            ),
            (
                Field::InputChannel,
                "Canal de entrada",
                format!("{} · mono", draft.input_channel + 1),
                inputs,
            ),
            (
                Field::Rate,
                "Sample rate",
                draft
                    .sample_rate
                    .map_or("Auto".into(), |n| format!("{n} Hz")),
                rates,
            ),
            (
                Field::Buffer,
                "Buffer size",
                draft
                    .buffer_frames
                    .map_or("Auto".into(), |n| format!("{n} samples")),
                buffers,
            ),
        ]
    }
}
