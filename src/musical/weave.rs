//! Regla Tejer preparada: selección constante por NoteOn, sin Lua ni reservas al consultar.
pub const MAX_STEPS: usize = 64;
pub const MAX_BRANCHES: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaveRule {
    steps: Vec<u8>,
}

impl WeaveRule {
    /// Secuencia visible con ramas numeradas desde 1; vacío vuelve al automático.
    pub fn parse_steps(input: &str) -> Result<Option<Self>, String> {
        let input = input.trim();
        if input.is_empty() || input.eq_ignore_ascii_case("auto") {
            return Ok(None);
        }
        let steps = input
            .split(|c: char| c == ',' || c.is_ascii_whitespace())
            .filter(|part| !part.is_empty())
            .map(|part| {
                let number = part.parse::<u8>().map_err(|_| {
                    "Tejer: escribe números de rama separados por comas".to_string()
                })?;
                number
                    .checked_sub(1)
                    .filter(|step| usize::from(*step) < MAX_BRANCHES)
                    .ok_or("Tejer: las ramas se numeran de 1 a 16".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::new(steps).map(Some)
    }
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

    #[test]
    fn visible_steps_parse_and_roundtrip_to_internal_indices() {
        let rule = WeaveRule::parse_steps("1, 3, 2").unwrap().unwrap();
        assert_eq!(rule.steps(), &[0, 2, 1]);
        assert_eq!(WeaveRule::parse_steps("auto").unwrap(), None);
        assert!(WeaveRule::parse_steps("0,1").is_err());
        assert!(WeaveRule::parse_steps("1,17").is_err());
    }
}
