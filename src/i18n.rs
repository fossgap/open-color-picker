//! Embedded application translations. GTK translates its own native controls.
use std::{collections::BTreeMap, sync::OnceLock};

macro_rules! messages {
    ($($key:ident => $english:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug)]
        pub enum Text { $($key),+ }
        impl Text {
            #[cfg(test)]
            const ALL: &'static [Self] = &[$(Self::$key),+];
            fn key(self) -> &'static str { match self { $(Self::$key => stringify!($key)),+ } }
            fn english(self) -> &'static str { match self { $(Self::$key => $english),+ } }
        }
    };
}

messages! {
    PickColor => "Pick a color",
    KeyboardShortcuts => "Keyboard shortcuts",
    HideToTray => "Hide to tray",
    About => "About",
    Quit => "Quit",
    ApplicationMenu => "Application menu",
    CurrentPreview => "Current color preview",
    AdjustColorEllipsis => "Adjust color…",
    SaveColor => "Save color",
    CopyFormat => "Copy {format}",
    EditHint => "Edit a value and press Enter to apply.",
    RecentEmpty => "Your recent colors appear here after your first pick.",
    FavoritesEmpty => "Save a color to keep it here for next time.",
    Recent => "Recent",
    Favorites => "Favorites",
    PickHint => "Pick a color from anywhere on your screen.",
    SaveColorsError => "Could not save your colors: {error}. They remain available until you quit.",
    Selected => "Selected {color}.",
    FavoriteSaved => "Color saved to Favorites.",
    FavoriteRemoved => "Color removed from Favorites.",
    CurrentColor => "Current color {color}",
    RemoveFavorite => "Remove favorite",
    SelectColor => "Select {color}",
    Copied => "Copied {value}.",
    AdjustColor => "Adjust color",
    PickingCancelled => "Picking cancelled.",
    WindowClosed => "The application window was closed.",
    SaveShortcutsError => "Could not save shortcuts to {path}: {error}. Check that its parent path is a writable folder, then choose Save again. Your previous shortcuts are still active.",
    ShortcutsSaved => "Keyboard shortcuts saved.",
    PickScreen => "Pick from your screen",
    PickScreenWithShortcut => "Pick from your screen ({shortcut})",
    NoTray => "No system tray is available. The window will stay open.",
    ShowColors => "Show colors",
    ChangeShortcut => "Change shortcut for {action}",
    ShortcutAccessible => "{action}: {shortcut}. Change shortcut",
    Cancel => "Cancel",
    Save => "Save",
    ShortcutInstructions => "Choose a shortcut to change or disable it. Save to apply your changes.",
    Application => "Application",
    WhilePicking => "While picking",
    RestoreDefaults => "Restore defaults",
    ShortcutFooter => "Tab navigation and focused buttons keep their standard GTK behavior. For a desktop-wide shortcut, bind open-color-picker --pick in your desktop’s keyboard settings.",
    ShortcutFor => "Shortcut for {action}",
    SetShortcut => "Set shortcut",
    RecorderInstructions => "Press your shortcut, then choose Set shortcut. Use Tab to reach the buttons.",
    PressShortcut => "Press a shortcut",
    NewShortcut => "New shortcut for {action}",
    DisableShortcut => "Disable shortcut",
    AboutComments => "Pick, copy, and keep colors from your Linux desktop.\nBuilt with Rust and GTK3.",
    ProjectWebsite => "Open Color Picker on GitHub",
    Contributors => "Fossgap contributors",
    LicenseNotice => "This project has not selected a license yet.\n\nIts dependencies are distributed under their own licenses. See the project repository for details.",
    BuiltWith => "Built with",
    PickPixelTitle => "Pick a pixel — Open Color Picker",
    PickPixel => "Pick a pixel",
    CapturedScreen => "Captured screen · original pixel values",
    UseColor => "Use color",
    CapturedImage => "Captured image: {description}",
    UseDescription => "Use {description}",
    CopyHex => "Copy HEX",
    CopyRgb => "Copy RGB",
    CopyHsl => "Copy HSL",
    ToggleFavorite => "Save or remove a favorite",
    MoveLeft => "Move left",
    MoveRight => "Move right",
    MoveUp => "Move up",
    MoveDown => "Move down",
    FastLeft => "Move left 10 pixels",
    FastRight => "Move right 10 pixels",
    FastUp => "Move up 10 pixels",
    FastDown => "Move down 10 pixels",
    CancelPicking => "Cancel picking",
    Disabled => "Disabled",
    ShortcutConflict => "This shortcut is already used by “{action}”. Choose another combination or disable that shortcut first.",
    PickerHelp => "Move one pixel: {left} / {right} / {up} / {down}\nMove ten pixels: {fast_left} / {fast_right} / {fast_up} / {fast_down}\nUse color: {confirm} · Cancel: {cancel} · Click a pixel to pick",
    ValidCombination => "Choose a valid key combination.",
    PressKey => "Press a key together with any modifiers you want to use.",
    TabReserved => "Tab is reserved for moving between controls.",
    CommandShortcut => "Use Ctrl, Alt or Super with a key, or choose a function key.",
    ReservedEditing => "This combination is reserved for editing text. Choose another shortcut.",
    ShortcutsLoadError => "Could not load keyboard shortcuts: {error}. Using defaults; the existing file will be backed up when you save shortcuts.",
    UnsupportedShortcuts => "unsupported shortcuts version",
    NoShortcutParent => "shortcuts have no parent directory",
    UnsupportedPalette => "unsupported palette version",
    NoPaletteParent => "palette has no parent directory",
    LoadColorsError => "Could not load saved colors: {error}. The existing file will be backed up before saving.",
    CliPick => "Pick a color immediately",
    CliTray => "Start in the system tray",
    CliQuit => "Quit the running application",
    CliVersion => "Print the version",
    ColorFormatError => "Use #3584E4, rgb(53, 132, 228), or hsl(213, 76%, 55%).",
    SessionError => "Could not connect to the desktop session: {error}",
    CaptureTimeout => "Screen capture timed out. Try again and respond to the desktop permission dialog.",
    InvalidCaptureRequest => "The desktop returned an invalid capture request.",
    CaptureUnavailable => "Screen capture is unavailable. Check xdg-desktop-portal and your desktop's Screenshot backend. {error}",
    ScreenshotOpenError => "Could not open the screenshot: {error}",
    NoScreenshot => "The desktop returned no local screenshot.",
    CaptureDenied => "Screen capture was denied or failed. Try again and allow the desktop capture request.",
}

const CATALOGS: [(&str, &str); 6] = [
    ("fr", include_str!("../data/i18n/fr.json")),
    ("es", include_str!("../data/i18n/es.json")),
    ("de", include_str!("../data/i18n/de.json")),
    ("pt", include_str!("../data/i18n/pt.json")),
    ("zh", include_str!("../data/i18n/zh_CN.json")),
    ("ja", include_str!("../data/i18n/ja.json")),
];

fn locale_language(locale: &str) -> Option<&'static str> {
    let locale = locale.trim().to_ascii_lowercase().replace('-', "_");
    let locale = locale.split(['.', '@']).next().unwrap_or("");
    let parts: Vec<_> = locale.split('_').collect();
    match parts[0] {
        "c" | "posix" | "en" => Some("en"),
        "fr" => Some("fr"),
        "es" => Some("es"),
        "de" => Some("de"),
        "pt" => Some("pt"),
        "ja" => Some("ja"),
        "zh" if !parts
            .iter()
            .any(|part| ["hant", "tw", "hk", "mo"].contains(part)) =>
        {
            Some("zh")
        }
        _ => None,
    }
}

fn select_language(
    language: Option<&str>,
    lc_all: Option<&str>,
    lc_messages: Option<&str>,
    lang: Option<&str>,
) -> &'static str {
    let locale = [lc_all, lc_messages, lang]
        .into_iter()
        .flatten()
        .find(|s| !s.is_empty())
        .unwrap_or("C");
    // Like gettext, an explicit C/POSIX locale suppresses translations.
    if locale_language(locale) == Some("en") && !locale.to_ascii_lowercase().starts_with("en") {
        return "en";
    }
    language
        .into_iter()
        .flat_map(|s| s.split(':'))
        .chain(std::iter::once(locale))
        .find_map(locale_language)
        .unwrap_or("en")
}

struct Catalog(BTreeMap<String, String>);

impl Catalog {
    fn new(language: &str) -> Self {
        Self(
            CATALOGS
                .iter()
                .find(|(code, _)| *code == language)
                .and_then(|(_, json)| serde_json::from_str(json).ok())
                .unwrap_or_default(),
        )
    }

    fn text(&self, message: Text) -> &str {
        self.0
            .get(message.key())
            .filter(|s| !s.is_empty())
            .map(String::as_str)
            .unwrap_or(message.english())
    }
}

static CATALOG: OnceLock<Catalog> = OnceLock::new();

pub fn tr(message: Text) -> &'static str {
    CATALOG
        .get_or_init(|| {
            let env = |name| std::env::var(name).ok();
            Catalog::new(select_language(
                env("LANGUAGE").as_deref(),
                env("LC_ALL").as_deref(),
                env("LC_MESSAGES").as_deref(),
                env("LANG").as_deref(),
            ))
        })
        .text(message)
}

pub fn trf(message: Text, arguments: &[(&str, &str)]) -> String {
    render(tr(message), arguments)
}

fn render(template: &str, arguments: &[(&str, &str)]) -> String {
    let mut output = String::new();
    let mut tail = template;
    while let Some((before, after)) = tail.split_once('{') {
        output.push_str(before);
        if let Some((name, rest)) = after.split_once('}') {
            if let Some((_, value)) = arguments.iter().find(|(key, _)| *key == name) {
                output.push_str(value);
            } else {
                output.push('{');
                output.push_str(name);
                output.push('}');
            }
            tail = rest;
        } else {
            output.push('{');
            tail = after;
            break;
        }
    }
    output.push_str(tail);
    output
}

/// Relocate native GTK/GLib translations in an AppImage. Normal installations
/// keep the system's domains. Call again after GTK initializes its own domain.
pub fn bind_native_catalogs() {
    let Some(directory) = std::env::var_os("OPEN_COLOR_PICKER_LOCALE_DIR") else {
        return;
    };
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let Ok(directory) = CString::new(directory.as_bytes()) else {
        return;
    };
    unsafe extern "C" {
        fn bindtextdomain(
            domain: *const std::ffi::c_char,
            directory: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_char;
        fn bind_textdomain_codeset(
            domain: *const std::ffi::c_char,
            codeset: *const std::ffi::c_char,
        ) -> *mut std::ffi::c_char;
    }
    for domain in [c"gtk30", c"gtk30-properties", c"glib20", c"atk10"] {
        // SAFETY: NUL-terminated strings live throughout both calls. This is
        // invoked on the main thread during startup, before creating widgets.
        unsafe {
            bindtextdomain(domain.as_ptr(), directory.as_ptr());
            bind_textdomain_codeset(domain.as_ptr(), c"UTF-8".as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn placeholders(text: &str) -> BTreeSet<&str> {
        text.split('{')
            .skip(1)
            .map(|part| part.split_once('}').expect("closed placeholder").0)
            .collect()
    }

    #[test]
    fn catalogs_are_complete_and_preserve_placeholders() {
        let expected: BTreeSet<_> = Text::ALL.iter().map(|key| key.key()).collect();
        for (language, json) in CATALOGS {
            let catalog: BTreeMap<String, String> = serde_json::from_str(json).unwrap();
            assert_eq!(
                catalog.keys().map(String::as_str).collect::<BTreeSet<_>>(),
                expected,
                "{language}"
            );
            for &key in Text::ALL {
                let translated = &catalog[key.key()];
                assert!(!translated.trim().is_empty(), "{language}: {key:?}");
                assert_eq!(
                    placeholders(translated),
                    placeholders(key.english()),
                    "{language}: {key:?}"
                );
                assert!(!translated.contains('\0'));
            }
        }
    }

    #[test]
    fn locale_priority_regions_and_fallback() {
        assert_eq!(
            select_language(Some("xx:fr:de"), None, None, Some("en_US.UTF-8")),
            "fr"
        );
        assert_eq!(
            select_language(None, Some("de_DE.UTF-8"), Some("es_ES"), Some("fr_FR")),
            "de"
        );
        assert_eq!(
            select_language(None, Some(""), Some("ja_JP.UTF-8"), Some("fr_FR")),
            "ja"
        );
        for locale in ["pt_BR.UTF-8", "pt-PT", "pt"] {
            assert_eq!(locale_language(locale), Some("pt"));
        }
        for locale in ["zh_CN.UTF-8", "zh-Hans-SG", "zh"] {
            assert_eq!(locale_language(locale), Some("zh"));
        }
        assert_eq!(select_language(None, None, None, Some("zh_Hant_TW")), "en");
        for locale in ["C", "POSIX", "C.UTF-8"] {
            assert_eq!(select_language(Some("fr"), Some(locale), None, None), "en");
        }
        assert_eq!(select_language(None, None, None, Some("xx_YY")), "en");
        assert_eq!(select_language(None, None, None, None), "en");
    }

    #[test]
    fn missing_translation_falls_back_and_values_are_not_reinterpreted() {
        assert_eq!(Catalog::new("xx").text(Text::PickColor), "Pick a color");
        assert_eq!(
            render(
                "{second} / {first}",
                &[("first", "{second}"), ("second", "✓")]
            ),
            "✓ / {second}"
        );
    }
}
