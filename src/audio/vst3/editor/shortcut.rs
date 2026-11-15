//! Estado de Espacio en X11; filtra la autorepetición KeyRelease/KeyPress.
#[derive(Default)]
pub(super) struct SpaceShortcut {
    held: bool,
    last_release: Option<u64>,
}

impl SpaceShortcut {
    pub fn key(&mut self, down: bool, time: u64) -> bool {
        if down {
            let toggle = !self.held && self.last_release != Some(time);
            self.held = true;
            toggle
        } else {
            self.held = false;
            self.last_release = Some(time);
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::SpaceShortcut;

    #[test]
    fn toggles_once_per_press_and_ignores_x11_repeat_pair() {
        let mut space = SpaceShortcut::default();
        assert!(space.key(true, 100));
        assert!(!space.key(true, 110));
        assert!(!space.key(false, 120));
        assert!(!space.key(true, 120));
        assert!(!space.key(false, 130));
        assert!(space.key(true, 200));
    }
}
