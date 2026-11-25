use super::{FxChain, FxId, FxKind, FxNode};

#[derive(Clone, Copy)]
pub struct Band {
    pub chain: FxId,
    pub input_crossover: FxId,
    pub crossover: FxId,
    pub hz: u16,
    pub low: u16,
    pub high: u16,
}

impl FxNode {
    pub fn crossover(hz: u16) -> Self {
        let branch = |name| {
            Self::new(
                name,
                FxKind::SerialChain {
                    children: Vec::new(),
                },
            )
        };
        Self::new(
            "Tejer EQ",
            FxKind::Crossover {
                hz,
                branches: vec![branch("Graves"), branch("Agudos")],
            },
        )
    }
}

impl FxChain {
    pub fn bands(&self) -> Vec<Band> {
        fn visit(nodes: &[FxNode], low: u16, high: u16, root: Option<FxId>, out: &mut Vec<Band>) {
            for node in nodes {
                if let FxKind::Crossover { hz, branches } = &node.kind {
                    let input = root.unwrap_or(node.id);
                    for (branch, (a, b)) in branches.iter().zip([(low, *hz), (*hz, high)]) {
                        let start = out.len();
                        visit(
                            branch.children().unwrap_or_default(),
                            a,
                            b,
                            Some(input),
                            out,
                        );
                        if out.len() == start {
                            out.push(Band {
                                chain: branch.id,
                                input_crossover: input,
                                crossover: node.id,
                                hz: *hz,
                                low: a,
                                high: b,
                            });
                        }
                    }
                } else if let Some(children) = node.children() {
                    visit(children, low, high, root, out);
                }
            }
        }
        let mut bands = Vec::new();
        visit(self.nodes(), 0, 24000, None, &mut bands);
        bands
    }
}

impl FxChain {
    pub fn accepts_cut_preview(&self, id: FxId, hz: u16) -> bool {
        validate_cuts(self.nodes(), 0, 24000, Some((id, hz))).is_ok()
    }
}

pub(super) fn validate(nodes: &[FxNode], low: u16, high: u16) -> Result<(), String> {
    validate_cuts(nodes, low, high, None)
}

fn validate_cuts(
    nodes: &[FxNode],
    low: u16,
    high: u16,
    preview: Option<(FxId, u16)>,
) -> Result<(), String> {
    for node in nodes {
        if let FxKind::Crossover { hz, branches } = &node.kind {
            let hz = preview
                .filter(|(id, _)| *id == node.id)
                .map_or(*hz, |(_, hz)| hz);
            if !(20..=20000).contains(&hz)
                || hz <= low
                || hz >= high
                || branches.len() != 2
                || !branches
                    .iter()
                    .all(|b| matches!(b.kind, FxKind::SerialChain { .. }))
            {
                return Err("Tejer EQ requiere dos cadenas y un corte dentro de la banda".into());
            }
            validate_cuts(branches[0].children().unwrap_or_default(), low, hz, preview)?;
            validate_cuts(
                branches[1].children().unwrap_or_default(),
                hz,
                high,
                preview,
            )?;
        } else if let Some(children) = node.children() {
            validate_cuts(children, low, high, preview)?;
        }
    }
    Ok(())
}
