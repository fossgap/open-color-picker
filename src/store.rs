use crate::color::Color;
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::{self, Write},
    path::PathBuf,
};

pub const HISTORY_LIMIT: usize = 24;

#[derive(Debug, Serialize, Deserialize)]
pub struct Palette {
    version: u8,
    pub current: Color,
    pub favorites: Vec<Color>,
    pub history: Vec<Color>,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            version: 1,
            current: Color::default(),
            favorites: vec![],
            history: vec![],
        }
    }
}

pub struct Store {
    pub palette: Palette,
    pub warning: Option<String>,
    path: PathBuf,
    needs_backup: bool,
}

impl Store {
    pub fn load() -> Self {
        let base = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| gtk::glib::home_dir().join(".local/share"));
        Self::at(base.join("open-color-picker/palette.json"))
    }
    pub fn at(path: PathBuf) -> Self {
        let mut store = Self {
            palette: Palette::default(),
            warning: None,
            path,
            needs_backup: false,
        };
        let result = fs::read(&store.path).and_then(|bytes| {
            let mut palette: Palette = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            if palette.version != 1 {
                return Err(io::Error::other("unsupported palette version"));
            }
            let mut unique = vec![];
            palette.favorites.retain(|c| {
                if unique.contains(c) {
                    false
                } else {
                    unique.push(*c);
                    true
                }
            });
            unique.clear();
            palette.history.retain(|c| {
                if unique.contains(c) {
                    false
                } else {
                    unique.push(*c);
                    true
                }
            });
            palette.history.truncate(HISTORY_LIMIT);
            Ok(palette)
        });
        match result {
            Ok(palette) => store.palette = palette,
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => {
                store.warning = Some(format!("Could not load saved colors: {error}. The existing file will be backed up before saving."));
                store.needs_backup = true;
            }
        }
        store
    }
    pub fn select(&mut self, color: Color) {
        self.palette.current = color;
        self.palette.history.retain(|c| *c != color);
        self.palette.history.insert(0, color);
        self.palette.history.truncate(HISTORY_LIMIT);
    }
    pub fn toggle_favorite(&mut self) {
        let current = self.palette.current;
        if self.palette.favorites.contains(&current) {
            self.palette.favorites.retain(|c| *c != current);
        } else {
            self.palette.favorites.push(current);
        }
    }
    pub fn save(&mut self) -> io::Result<()> {
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("palette has no parent directory"))?;
        fs::create_dir_all(parent)?;
        if self.needs_backup && self.path.exists() {
            let mut backup = tempfile::Builder::new()
                .prefix("palette-backup-")
                .suffix(".json")
                .tempfile_in(parent)?;
            backup.write_all(&fs::read(&self.path)?)?;
            backup.as_file().sync_all()?;
            backup.keep().map_err(io::Error::other)?;
            self.needs_backup = false;
        }
        let mut temp = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(&mut temp, &self.palette)?;
        temp.write_all(b"\n")?;
        temp.as_file().sync_all()?;
        temp.persist(&self.path).map_err(io::Error::other)?;
        self.warning = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn persists_deduplicated_history_and_favorites() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("nested/palette.json");
        let mut store = Store::at(path.clone());
        for r in 0..40 {
            store.select(Color(r, 0, 0));
        }
        store.select(Color(10, 0, 0));
        store.toggle_favorite();
        store.save().unwrap();
        let reloaded = Store::at(path);
        assert_eq!(reloaded.palette.current, Color(10, 0, 0));
        assert_eq!(reloaded.palette.history.len(), HISTORY_LIMIT);
        assert_eq!(reloaded.palette.history[0], Color(10, 0, 0));
        assert_eq!(reloaded.palette.favorites, [Color(10, 0, 0)]);
    }
    #[test]
    fn preserves_corrupt_file_before_recovery() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("palette.json");
        fs::write(&path, "broken data").unwrap();
        let mut store = Store::at(path.clone());
        assert!(store.warning.is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken data");
        store.save().unwrap();
        assert!(fs::read_dir(temp.path())
            .unwrap()
            .filter_map(Result::ok)
            .any(|e| e
                .file_name()
                .to_string_lossy()
                .starts_with("palette-backup-")
                && fs::read_to_string(e.path()).unwrap() == "broken data"));
        assert!(Store::at(path).warning.is_none());
    }
    #[test]
    fn reports_write_failures() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("not-a-directory");
        fs::write(&file, "x").unwrap();
        assert!(Store::at(file.join("palette.json")).save().is_err());
    }
}
