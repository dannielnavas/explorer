use std::path::{Path, PathBuf};

/// Historial de navegación estilo navegador (Atrás / Adelante) por pestaña.
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    entries: Vec<PathBuf>,
    cursor: usize,
}

impl NavigationHistory {
    pub fn new(initial: PathBuf) -> Self {
        Self {
            entries: vec![initial],
            cursor: 0,
        }
    }

    pub fn current(&self) -> &Path {
        &self.entries[self.cursor]
    }

    pub fn can_go_back(&self) -> bool {
        self.cursor > 0
    }

    pub fn can_go_forward(&self) -> bool {
        self.cursor + 1 < self.entries.len()
    }

    /// Navega a una nueva ruta, descartando el historial futuro si el cursor no está al final.
    pub fn push(&mut self, path: PathBuf) {
        if self.current() == path {
            return;
        }
        self.entries.truncate(self.cursor + 1);
        self.entries.push(path);
        self.cursor = self.entries.len() - 1;
    }

    pub fn go_back(&mut self) -> Option<&Path> {
        if self.can_go_back() {
            self.cursor -= 1;
            Some(self.current())
        } else {
            None
        }
    }

    pub fn go_forward(&mut self) -> Option<&Path> {
        if self.can_go_forward() {
            self.cursor += 1;
            Some(self.current())
        } else {
            None
        }
    }
}
