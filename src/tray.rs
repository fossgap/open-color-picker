use crate::i18n::{tr, Text};
use gtk::{gio::prelude::*, glib};
use ksni::blocking::TrayMethods;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub struct Tray {
    handle: ksni::blocking::Handle<Item>,
    available: Arc<AtomicBool>,
}

impl Tray {
    pub fn new(app: &gtk::Application) -> Result<Self, ksni::Error> {
        let available = Arc::new(AtomicBool::new(true));
        let handle = Item {
            app: app.downgrade().into(),
            available: available.clone(),
        }
        // Keep watching if the desktop's panel starts after the application.
        // The window stays visible until a tray host actually registers us.
        .assume_sni_available(true)
        .spawn()?;
        Ok(Self { handle, available })
    }

    pub fn is_available(&self) -> bool {
        self.available.load(Ordering::Acquire) && !self.handle.is_closed()
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        self.handle.shutdown().wait();
    }
}

struct Item {
    app: glib::SendWeakRef<gtk::Application>,
    available: Arc<AtomicBool>,
}

impl Item {
    fn dispatch(&self, action: &'static str) {
        let app = self.app.clone();
        // The D-Bus service runs on its own thread; GTK actions must run on
        // the main thread, including callbacks from the exported tray menu.
        glib::idle_add_once(move || {
            if let Some(app) = app.upgrade() {
                app.activate_action(action, None);
            }
        });
    }
}

impl ksni::Tray for Item {
    // Advertise ItemIsMenu=false so hosts distinguish left click from menu.
    const MENU_ON_ACTIVATE: bool = false;

    fn id(&self) -> String {
        crate::APP_ID.into()
    }

    fn title(&self) -> String {
        "Open Color Picker".into()
    }

    fn icon_name(&self) -> String {
        "color-select-symbolic".into()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        self.dispatch("pick");
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        let item = |label: &str, action| {
            ksni::menu::StandardItem {
                label: label.into(),
                activate: Box::new(move |item: &mut Self| item.dispatch(action)),
                ..Default::default()
            }
            .into()
        };
        vec![
            item(tr(Text::PickColor), "pick"),
            item(tr(Text::ShowColors), "show"),
            ksni::MenuItem::Separator,
            item(tr(Text::Quit), "quit"),
        ]
    }

    fn watcher_online(&self) {
        self.available.store(true, Ordering::Release);
    }

    fn watcher_offline(&self, _reason: ksni::OfflineReason) -> bool {
        self.available.store(false, Ordering::Release);
        self.dispatch("tray-unavailable");
        true
    }
}
