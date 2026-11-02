use crate::i18n::{tr, trf, Text};
use gtk::gdk::{self, keys::constants as key, ModifierType as Mods};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Write},
    path::PathBuf,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Action {
    Pick,
    CopyHex,
    CopyRgb,
    CopyHsl,
    Favorite,
    Adjust,
    Shortcuts,
    Hide,
    About,
    Quit,
    Left,
    Right,
    Up,
    Down,
    FastLeft,
    FastRight,
    FastUp,
    FastDown,
    Confirm,
    Cancel,
}

pub const APPLICATION: [Action; 10] = [
    Action::Pick,
    Action::CopyHex,
    Action::CopyRgb,
    Action::CopyHsl,
    Action::Favorite,
    Action::Adjust,
    Action::Shortcuts,
    Action::Hide,
    Action::About,
    Action::Quit,
];
pub const PICKER: [Action; 10] = [
    Action::Left,
    Action::Right,
    Action::Up,
    Action::Down,
    Action::FastLeft,
    Action::FastRight,
    Action::FastUp,
    Action::FastDown,
    Action::Confirm,
    Action::Cancel,
];

impl Action {
    pub fn id(self) -> &'static str {
        match self {
            Self::Pick => "pick",
            Self::CopyHex => "copy-hex",
            Self::CopyRgb => "copy-rgb",
            Self::CopyHsl => "copy-hsl",
            Self::Favorite => "favorite",
            Self::Adjust => "adjust",
            Self::Shortcuts => "shortcuts",
            Self::Hide => "hide",
            Self::About => "about",
            Self::Quit => "quit",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
            Self::FastLeft => "fast-left",
            Self::FastRight => "fast-right",
            Self::FastUp => "fast-up",
            Self::FastDown => "fast-down",
            Self::Confirm => "confirm",
            Self::Cancel => "cancel",
        }
    }
    pub fn title(self) -> &'static str {
        match self {
            Self::Pick => tr(Text::PickColor),
            Self::CopyHex => tr(Text::CopyHex),
            Self::CopyRgb => tr(Text::CopyRgb),
            Self::CopyHsl => tr(Text::CopyHsl),
            Self::Favorite => tr(Text::ToggleFavorite),
            Self::Adjust => tr(Text::AdjustColor),
            Self::Shortcuts => tr(Text::KeyboardShortcuts),
            Self::Hide => tr(Text::HideToTray),
            Self::About => tr(Text::About),
            Self::Quit => tr(Text::Quit),
            Self::Left => tr(Text::MoveLeft),
            Self::Right => tr(Text::MoveRight),
            Self::Up => tr(Text::MoveUp),
            Self::Down => tr(Text::MoveDown),
            Self::FastLeft => tr(Text::FastLeft),
            Self::FastRight => tr(Text::FastRight),
            Self::FastUp => tr(Text::FastUp),
            Self::FastDown => tr(Text::FastDown),
            Self::Confirm => tr(Text::UseColor),
            Self::Cancel => tr(Text::CancelPicking),
        }
    }
    fn default_accel(self) -> &'static str {
        match self {
            Self::Pick => "<Primary>p",
            Self::CopyHex => "<Primary><Shift>c",
            Self::Favorite => "<Primary>s",
            Self::Shortcuts => "F1",
            Self::Quit => "<Primary>q",
            Self::Left => "Left",
            Self::Right => "Right",
            Self::Up => "Up",
            Self::Down => "Down",
            Self::FastLeft => "<Shift>Left",
            Self::FastRight => "<Shift>Right",
            Self::FastUp => "<Shift>Up",
            Self::FastDown => "<Shift>Down",
            Self::Confirm => "Return",
            Self::Cancel => "Escape",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bindings(BTreeMap<Action, String>);

impl Default for Bindings {
    fn default() -> Self {
        Self(
            APPLICATION
                .into_iter()
                .chain(PICKER)
                .map(|action| (action, action.default_accel().into()))
                .collect(),
        )
    }
}

impl Bindings {
    pub fn accelerator(&self, action: Action) -> &str {
        &self.0[&action]
    }
    pub fn label(&self, action: Action) -> String {
        let value = self.accelerator(action);
        if value.is_empty() {
            return tr(Text::Disabled).into();
        }
        let (key, mods) = gtk::accelerator_parse(value);
        gtk::accelerator_get_label(key, mods)
            .map(|v| v.to_string())
            .unwrap_or_else(|| value.into())
    }
    pub fn set(&mut self, action: Action, value: &str) -> Result<(), String> {
        let value = canonical(action, value)?;
        if !value.is_empty() {
            let binding = gtk::accelerator_parse(&value);
            for (&other, existing) in &self.0 {
                if other != action
                    && !existing.is_empty()
                    && gtk::accelerator_parse(existing) == binding
                {
                    return Err(trf(Text::ShortcutConflict, &[("action", other.title())]));
                }
            }
        }
        self.0.insert(action, value);
        Ok(())
    }
    pub fn matches(&self, action: Action, event: &gdk::EventKey) -> bool {
        let value = self.accelerator(action);
        !value.is_empty() && gtk::accelerator_parse(value) == event_key(event)
    }
    pub fn picker_help(&self) -> String {
        trf(
            Text::PickerHelp,
            &[
                ("left", &self.label(Action::Left)),
                ("right", &self.label(Action::Right)),
                ("up", &self.label(Action::Up)),
                ("down", &self.label(Action::Down)),
                ("fast_left", &self.label(Action::FastLeft)),
                ("fast_right", &self.label(Action::FastRight)),
                ("fast_up", &self.label(Action::FastUp)),
                ("fast_down", &self.label(Action::FastDown)),
                ("confirm", &self.label(Action::Confirm)),
                ("cancel", &self.label(Action::Cancel)),
            ],
        )
    }
    fn validate(&self) -> Result<(), String> {
        let mut checked = self.clone();
        for (&action, value) in &self.0 {
            checked.set(action, value)?;
        }
        Ok(())
    }
}

pub fn event_key(event: &gdk::EventKey) -> (u32, Mods) {
    let key = if event.keyval() == key::KP_Enter {
        key::Return
    } else {
        event.keyval().to_lower()
    };
    (*key, normalize_modifiers(event.state()))
}

fn normalize_modifiers(mut modifiers: Mods) -> Mods {
    if let Some(keymap) =
        gdk::Display::default().and_then(|display| gdk::Keymap::for_display(&display))
    {
        keymap.map_virtual_modifiers(&mut modifiers);
        keymap.add_virtual_modifiers(&mut modifiers);
        // Some X11 layouts map Super and Hyper onto the same physical modifier.
        // GTK cannot activate a binding containing both aliases. Prefer Alt
        // and Super, and retain a single name for each physical modifier.
        let physical_mask =
            Mods::MOD1_MASK | Mods::MOD2_MASK | Mods::MOD3_MASK | Mods::MOD4_MASK | Mods::MOD5_MASK;
        let mut claimed = modifiers & Mods::MOD1_MASK;
        for virtual_modifier in [Mods::SUPER_MASK, Mods::HYPER_MASK, Mods::META_MASK] {
            if !modifiers.contains(virtual_modifier) {
                continue;
            }
            let mut physical = virtual_modifier;
            keymap.map_virtual_modifiers(&mut physical);
            physical &= physical_mask;
            if !physical.is_empty() && claimed.intersects(physical) {
                modifiers.remove(virtual_modifier);
            } else {
                claimed |= physical;
            }
        }
    }
    modifiers & gtk::accelerator_get_default_mod_mask()
}

pub fn from_event(event: &gdk::EventKey) -> Option<String> {
    if event.is_modifier() {
        return None;
    }
    let (key, mods) = event_key(event);
    gtk::accelerator_name(key, mods).map(|value| value.to_string())
}

fn canonical(action: Action, value: &str) -> Result<String, String> {
    if value.is_empty() {
        return Ok(String::new());
    }
    if value.contains('\0') {
        return Err(tr(Text::ValidCombination).into());
    }
    let (raw_key, mods) = gtk::accelerator_parse(value);
    let key = gdk::keys::Key::from(raw_key).to_lower();
    let key = if key == key::KP_Enter {
        key::Return
    } else {
        key
    };
    // GTK's application-accelerator validator excludes navigation keys. They
    // are intentional bindings on the picker's focused image canvas.
    let picker_navigation = PICKER.contains(&action)
        && matches!(
            key,
            key::Left
                | key::Right
                | key::Up
                | key::Down
                | key::Home
                | key::End
                | key::Page_Up
                | key::Page_Down
                | key::Return
                | key::Escape
                | key::space
        );
    if raw_key == 0
        || (!gtk::accelerator_valid(*key, mods) && !picker_navigation)
        || !(mods - gtk::accelerator_get_default_mod_mask()).is_empty()
    {
        return Err(tr(Text::PressKey).into());
    }
    if matches!(key, key::Tab | key::ISO_Left_Tab) {
        return Err(tr(Text::TabReserved).into());
    }
    if APPLICATION.contains(&action) {
        let command = Mods::CONTROL_MASK
            | Mods::MOD1_MASK
            | Mods::SUPER_MASK
            | Mods::META_MASK
            | Mods::HYPER_MASK;
        if !(mods.intersects(command) || key >= key::F1 && key <= key::F35) {
            return Err(tr(Text::CommandShortcut).into());
        }
        let editing = mods == Mods::CONTROL_MASK
            && matches!(
                key,
                key::a | key::c | key::v | key::x | key::z | key::y | key::Insert
            )
            || mods == (Mods::CONTROL_MASK | Mods::SHIFT_MASK) && key == key::z;
        if editing {
            return Err(tr(Text::ReservedEditing).into());
        }
    }
    gtk::accelerator_name(*key, normalize_modifiers(mods))
        .map(|value| value.to_string())
        .ok_or_else(|| tr(Text::ValidCombination).into())
}

#[derive(Serialize, Deserialize)]
struct Document {
    version: u8,
    bindings: BTreeMap<Action, String>,
}

pub struct Store {
    pub bindings: Bindings,
    pub warning: Option<String>,
    path: PathBuf,
    needs_backup: bool,
}

impl Store {
    pub fn path(&self) -> &std::path::Path {
        &self.path
    }
    pub fn load() -> Self {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| gtk::glib::home_dir().join(".config"));
        Self::at(base.join("open-color-picker/shortcuts.json"))
    }
    pub fn at(path: PathBuf) -> Self {
        let mut store = Self {
            bindings: Bindings::default(),
            warning: None,
            path,
            needs_backup: false,
        };
        let loaded = fs::read(&store.path).and_then(|bytes| {
            let document: Document = serde_json::from_slice(&bytes).map_err(io::Error::other)?;
            if document.version != 1 {
                return Err(io::Error::other(tr(Text::UnsupportedShortcuts)));
            }
            let mut bindings = Bindings::default();
            // Load the whole map before validation, so swaps do not conflict
            // with defaults that another entry in the file replaces.
            for (action, value) in document.bindings {
                bindings
                    .0
                    .insert(action, canonical(action, &value).map_err(io::Error::other)?);
            }
            bindings.validate().map_err(io::Error::other)?;
            Ok(bindings)
        });
        match loaded {
            Ok(bindings) => store.bindings = bindings,
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => {
                store.warning = Some(trf(
                    Text::ShortcutsLoadError,
                    &[("error", &error.to_string())],
                ));
                store.needs_backup = true;
            }
        }
        store
    }
    pub fn save(&mut self, bindings: &Bindings) -> io::Result<()> {
        bindings.validate().map_err(io::Error::other)?;
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other(tr(Text::NoShortcutParent)))?;
        fs::create_dir_all(parent)?;
        if self.needs_backup && self.path.exists() {
            let mut backup = tempfile::Builder::new()
                .prefix("shortcuts-backup-")
                .suffix(".json")
                .tempfile_in(parent)?;
            backup.write_all(&fs::read(&self.path)?)?;
            backup.as_file().sync_all()?;
            backup.keep().map_err(io::Error::other)?;
            self.needs_backup = false;
        }
        let mut file = tempfile::NamedTempFile::new_in(parent)?;
        serde_json::to_writer_pretty(
            &mut file,
            &Document {
                version: 1,
                bindings: bindings.0.clone(),
            },
        )?;
        file.write_all(b"\n")?;
        file.as_file().sync_all()?;
        file.persist(&self.path).map_err(io::Error::other)?;
        self.bindings = bindings.clone();
        self.warning = None;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires GTK; run in the private Xvfb test session"]
    fn shortcuts_configuration() {
        gtk::init().unwrap();
        let mut bindings = Bindings::default();
        bindings.validate().unwrap();
        let original = bindings.clone();
        for invalid in [
            "not-a-key",
            "<Primary>",
            "p",
            "<Primary>c",
            "Tab",
            "<Primary>Tab",
        ] {
            assert!(bindings.set(Action::Pick, invalid).is_err(), "{invalid}");
            assert_eq!(bindings, original);
        }
        assert!(bindings.set(Action::Pick, "<Control>s").is_err());
        bindings.set(Action::Pick, "<Primary><Alt>p").unwrap();
        assert!(bindings.set(Action::CopyHex, "<Control><Alt>p").is_err());
        bindings.set(Action::CopyHex, "").unwrap();
        bindings.set(Action::Right, "d").unwrap();
        assert!(bindings.set(Action::Left, "d").is_err());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config/shortcuts.json");
        let mut store = Store::at(path.clone());
        store.save(&bindings).unwrap();
        let loaded = Store::at(path.clone());
        assert_eq!(loaded.bindings.accelerator(Action::CopyHex), "");
        assert_eq!(loaded.bindings.accelerator(Action::Right), "d");
        assert_eq!(
            loaded.bindings.label(Action::Pick),
            bindings.label(Action::Pick)
        );
        fs::write(&path, "broken configuration").unwrap();
        let mut corrupt = Store::at(path.clone());
        assert!(corrupt.warning.is_some());
        assert_eq!(corrupt.bindings, Bindings::default());
        corrupt.save(&bindings).unwrap();
        let backups: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("shortcuts-backup-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        assert_eq!(
            fs::read_to_string(backups[0].path()).unwrap(),
            "broken configuration"
        );
        // A failed save must leave live bindings unchanged.
        let blocker = dir.path().join("file");
        fs::write(&blocker, "not a directory").unwrap();
        let mut blocked = Store::at(blocker.join("shortcuts.json"));
        assert!(blocked.save(&bindings).is_err());
        assert_eq!(blocked.bindings, Bindings::default());
        // Malformed, conflicting and newer schemas all fail safely.
        for document in [
            r#"{"version":2,"bindings":{}}"#,
            r#"{"version":1,"bindings":{"pick":"<Primary>s"}}"#,
            r#"{"version":1,"bindings":{"pick":"bad key"}}"#,
        ] {
            fs::write(&path, document).unwrap();
            assert!(Store::at(path.clone()).warning.is_some());
        }
    }
}
