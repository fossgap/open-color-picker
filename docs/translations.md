# Translations

English is the fallback language. Application catalogs live in `data/i18n/`:

| Language | Catalog |
| --- | --- |
| French | `fr.json` |
| Spanish | `es.json` |
| German | `de.json` |
| Portuguese | `pt.json` |
| Simplified Chinese | `zh_CN.json` |
| Japanese | `ja.json` |

The application reads the desktop's `LANGUAGE` preference list, then the
effective locale (`LC_ALL`, `LC_MESSAGES`, or `LANG`, in that order). Regional
variants such as `fr_CA`, `es_MX`, and `pt_BR` use the matching language catalog.
Simplified Chinese supports `zh_CN` and `zh_Hans`; Traditional Chinese locales
fall back to English. The C/POSIX locale always selects English. Restart the
application after changing languages, since it runs as a single instance.

To try French on a system with the `fr_FR.UTF-8` locale installed:

```sh
cargo run --locked -- --quit
LC_ALL=fr_FR.UTF-8 LANGUAGE=fr cargo run --locked
```

## Editing a translation

Keys and English text are defined in `src/i18n.rs`. Keep every catalog's keys
in sync and preserve named placeholders such as `{action}` and `{color}`;
their order can change. Do not translate the product name, command-line flags,
color syntax, or saved shortcut identifiers. Native GTK controls and key names
use the system's GTK translations. AppImage includes those catalogs too.

For a new message, add its typed `Text` key and English fallback, fill in all
six catalogs, and use `tr` or `trf` at the call site. Desktop launcher comments,
keywords, and the Pick action are translated separately in
`data/io.github.opencolorpicker.OpenColorPicker.desktop`.

`cargo test --locked i18n::tests` verifies complete catalogs, matching
placeholders, locale precedence, and fallback behavior. `./scripts/ci.sh check`
also runs the actual GTK interface in each language, checking translated
controls, shortcut conflicts, About, and unchanged color values. The Docker
image provides all six locales and fonts for Chinese and Japanese.

For local GUI tests, install the dependencies and generate the locales listed
in `Dockerfile`, then run `./scripts/check.sh`. Optional screenshots are saved
outside the source tree when `OCP_UI_CAPTURE_DIR` is set; locale tests put each
language in its own subdirectory. Set `GTK_THEME=Adwaita:dark` to review dark mode.
