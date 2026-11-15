//! Regla Tejer preparada: selección constante por NoteOn, sin Lua ni reservas al consultar.
pub const MAX_STEPS: usize = 64;
pub const MAX_BRANCHES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaveRule {
    steps: Vec<u8>,
}

impl WeaveRule {
    /// Los pasos internos son índices de rama con base cero.
    pub fn new(steps: Vec<u8>) -> Result<Self, String> {
        if steps.is_empty()
            || steps.len() > MAX_STEPS
            || steps.iter().any(|&step| usize::from(step) >= MAX_BRANCHES)
        {
            return Err("Tejer requiere 1–64 pasos entre las primeras 16 ramas".into());
        }
        Ok(Self { steps })
    }

    pub fn valid_for(&self, branches: usize) -> bool {
        (2..=MAX_BRANCHES).contains(&branches)
            && self.steps.iter().all(|&step| usize::from(step) < branches)
    }

    pub fn branch_at(&self, note_on_number: usize) -> usize {
        usize::from(self.steps[note_on_number % self.steps.len()])
    }

    pub fn steps(&self) -> &[u8] {
        &self.steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_cycles_without_allocating_or_using_a_clock() {
        let rule = WeaveRule::new(vec![0, 2, 1]).unwrap();
        assert!(rule.valid_for(3));
        assert!(!rule.valid_for(2));
        assert_eq!(
            (0..6).map(|n| rule.branch_at(n)).collect::<Vec<_>>(),
            vec![0, 2, 1, 0, 2, 1]
        );
    }
}
