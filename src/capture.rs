use gtk::{gdk, gdk_pixbuf::Pixbuf, gio, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
    time::Duration,
};

const DESTINATION: &str = "org.freedesktop.portal.Desktop";
const DESKTOP: &str = "/org/freedesktop/portal/desktop";
const REQUEST: &str = "org.freedesktop.portal.Request";
type CaptureResult = Result<Option<Pixbuf>, String>;
type Callback = Box<dyn FnOnce(CaptureResult)>;

pub fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v == "wayland")
        || gdk::Display::default().is_some_and(|d| d.type_().name().contains("Wayland"))
}

pub fn capture(callback: impl FnOnce(CaptureResult) + 'static) -> Option<Rc<PortalCapture>> {
    if !is_wayland() {
        let root = gdk::Window::default_root_window();
        if let Some(image) = root.pixbuf(0, 0, root.width(), root.height()) {
            callback(Ok(Some(image)));
            return None;
        }
    }
    match gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) {
        Ok(bus) => Some(PortalCapture::start(bus, Box::new(callback))),
        Err(error) => {
            callback(Err(format!(
                "Could not connect to the desktop session: {error}"
            )));
            None
        }
    }
}

pub struct PortalCapture {
    bus: gio::DBusConnection,
    path: RefCell<String>,
    subscription: RefCell<Option<gio::SignalSubscriptionId>>,
    timeout: RefCell<Option<glib::SourceId>>,
    callback: RefCell<Option<Callback>>,
    done: Cell<bool>,
}

impl PortalCapture {
    fn start(bus: gio::DBusConnection, callback: Callback) -> Rc<Self> {
        let token = format!("ocp_{}", glib::uuid_string_random().replace('-', "_"));
        let sender = bus
            .unique_name()
            .expect("session bus has a unique name")
            .trim_start_matches(':')
            .replace('.', "_");
        let request = Rc::new(Self {
            bus,
            path: RefCell::new(format!("{DESKTOP}/request/{sender}/{token}")),
            subscription: RefCell::new(None),
            timeout: RefCell::new(None),
            callback: RefCell::new(Some(callback)),
            done: Cell::new(false),
        });
        request.subscribe();
        let weak = Rc::downgrade(&request);
        *request.timeout.borrow_mut() = Some(glib::timeout_add_local_once(
            Duration::from_secs(120),
            move || {
                if let Some(request) = weak.upgrade() {
                    request.timeout.borrow_mut().take();
                    request.close(&request.path.borrow());
                    request.finish(Err("Screen capture timed out. Try again and respond to the desktop permission dialog.".into()));
                }
            },
        ));
        let options: HashMap<&str, glib::Variant> = HashMap::from([
            ("handle_token", token.to_variant()),
            ("interactive", false.to_variant()),
        ]);
        let parameters = ("", options).to_variant();
        let cloned = request.clone();
        request.bus.call(Some(DESTINATION), DESKTOP, "org.freedesktop.portal.Screenshot", "Screenshot",
            Some(&parameters), None, gio::DBusCallFlags::NONE, 120_000, gio::Cancellable::NONE, move |result| {
                match result {
                    Ok(reply) => {
                        // A D-Bus object path has type `o`, not `s`.
                        let path = reply.child_value(0).str().map(str::to_owned);
                        if let Some(path) = path {
                            if cloned.done.get() { cloned.close(&path); }
                            else if *cloned.path.borrow() != path {
                                *cloned.path.borrow_mut() = path;
                                cloned.subscribe();
                            }
                        } else { cloned.finish(Err("The desktop returned an invalid capture request.".into())); }
                    }
                    Err(error) => cloned.finish(Err(format!("Screen capture is unavailable. Check xdg-desktop-portal and your desktop's Screenshot backend. {error}"))),
                }
            });
        request
    }
    fn subscribe(self: &Rc<Self>) {
        if let Some(id) = self.subscription.borrow_mut().take() {
            self.bus.signal_unsubscribe(id);
        }
        let weak = Rc::downgrade(self);
        let id = self.bus.signal_subscribe(Some(DESTINATION), Some(REQUEST), Some("Response"), Some(&self.path.borrow()), None,
            gio::DBusSignalFlags::NONE, move |_, _, _, _, _, parameters| {
                if let Some(request) = weak.upgrade() {
                    if request.done.get() { return; }
                    let result = parameters.get::<(u32, HashMap<String, glib::Variant>)>();
                    match result {
                        Some((0, results)) => {
                            let path = results.get("uri").and_then(|v| v.get::<String>()).and_then(|uri| gio::File::for_uri(&uri).path());
                            request.finish(match path {
                                Some(path) => Pixbuf::from_file(path).map(Some).map_err(|e| format!("Could not open the screenshot: {e}")),
                                None => Err("The desktop returned no local screenshot.".into()),
                            });
                        }
                        Some((1, _)) => request.finish(Ok(None)),
                        _ => request.finish(Err("Screen capture was denied or failed. Try again and allow the desktop capture request.".into())),
                    }
                }
            });
        *self.subscription.borrow_mut() = Some(id);
    }
    fn close(&self, path: &str) {
        self.bus.call(
            Some(DESTINATION),
            path,
            REQUEST,
            "Close",
            None,
            None,
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
            |_| {},
        );
    }
    pub fn cancel(&self) {
        if !self.done.get() {
            self.close(&self.path.borrow());
            self.finish(Ok(None));
        }
    }
    fn finish(&self, result: CaptureResult) {
        if self.done.replace(true) {
            return;
        }
        if let Some(id) = self.subscription.borrow_mut().take() {
            self.bus.signal_unsubscribe(id);
        }
        if let Some(id) = self.timeout.borrow_mut().take() {
            id.remove();
        }
        let callback = self.callback.borrow_mut().take();
        if let Some(callback) = callback {
            callback(result);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    };

    #[test]
    #[ignore = "run in a private session with dbus-run-session"]
    fn portal_round_trip() {
        let context = glib::MainContext::default();
        let _guard = context.acquire().unwrap();
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
        let reply = bus
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "RequestName",
                Some(&(DESTINATION, 4u32).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                1000,
                gio::Cancellable::NONE,
            )
            .unwrap();
        assert_eq!(
            reply.get::<(u32,)>(),
            Some((1,)),
            "Run this test inside dbus-run-session, away from a real portal"
        );
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("capture.png");
        let image = Pixbuf::new(gtk::gdk_pixbuf::Colorspace::Rgb, false, 8, 2, 2).unwrap();
        image.fill(0x1a2b3cff);
        image.savev(&path, "png", &[]).unwrap();
        let uri = gio::File::for_path(path).uri().to_string();
        let mode = Arc::new(AtomicU32::new(0));
        let closed = Arc::new(AtomicU32::new(0));
        let node = gio::DBusNodeInfo::for_xml(r#"<node><interface name="org.freedesktop.portal.Screenshot"><method name="Screenshot"><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method></interface></node>"#).unwrap();
        let handler_mode = mode.clone();
        let handler_closed = closed.clone();
        let registration = bus.register_object(DESKTOP, &node.interfaces()[0], move |connection, sender, _, _, _, parameters, invocation| {
            let mode = handler_mode.load(Ordering::SeqCst);
            if mode == 4 { invocation.return_dbus_error("org.freedesktop.portal.Error.NotAllowed", "Denied by test portal"); return; }
            let (_, options) = parameters.get::<(String, HashMap<String, glib::Variant>)>().unwrap();
            let token = options["handle_token"].get::<String>().unwrap();
            let path = format!("{DESKTOP}/request/{}/{}{}", sender.trim_start_matches(':').replace('.', "_"), token, if mode == 6 { "_legacy" } else { "" });
            let request_node = gio::DBusNodeInfo::for_xml(r#"<node><interface name="org.freedesktop.portal.Request"><method name="Close"/></interface></node>"#).unwrap();
            let closed = handler_closed.clone();
            let _registration = connection.register_object(&path, &request_node.interfaces()[0], move |_, _, _, _, _, _, invocation| {
                closed.fetch_add(1, Ordering::SeqCst); invocation.return_value(None);
            }, |_, _, _, _, _| ().to_variant(), |_, _, _, _, _, _| false).unwrap();
            let mut results = HashMap::<String,glib::Variant>::new();
            if mode == 0 || mode == 6 { results.insert("uri".into(),uri.to_variant()); }
            let response = (if mode < 3 { mode } else { 0 }, results).to_variant();
            let reply = (glib::variant::ObjectPath::try_from(path.as_str()).unwrap(),).to_variant();
            if mode == 6 {
                invocation.return_value(Some(&reply));
                let sender = sender.to_owned();
                glib::timeout_add_local_once(Duration::from_millis(30), move || {
                    connection.emit_signal(Some(&sender), &path, REQUEST, "Response", Some(&response)).unwrap();
                });
            } else {
                // Emit before the method reply to exercise the subscribe-before-call race.
                if mode != 5 { connection.emit_signal(Some(sender), &path, REQUEST, "Response", Some(&response)).unwrap(); }
                invocation.return_value(Some(&reply));
            }
        }, |_, _, _, _, _| ().to_variant(), |_, _, _, _, _, _| false).unwrap();
        for scenario in 0..=6 {
            mode.store(scenario, Ordering::SeqCst);
            let result = Rc::new(RefCell::new(None));
            let delivered = result.clone();
            let request = PortalCapture::start(
                bus.clone(),
                Box::new(move |value| *delivered.borrow_mut() = Some(value)),
            );
            if scenario == 5 {
                let request = request.clone();
                glib::timeout_add_local_once(Duration::from_millis(30), move || request.cancel());
            }
            let deadline = std::time::Instant::now() + Duration::from_secs(3);
            while result.borrow().is_none() && std::time::Instant::now() < deadline {
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let result = result
                .borrow_mut()
                .take()
                .expect("portal callback must resolve");
            match scenario {
                0 | 6 => {
                    let pixels = crate::pixels::Pixels::new(&result.unwrap().unwrap());
                    assert_eq!(pixels.at(1, 1), crate::color::Color(26, 43, 60));
                }
                1 | 5 => assert!(result.unwrap().is_none()),
                _ => assert!(result.is_err()),
            }
            assert!(request.done.get());
            assert!(request.subscription.borrow().is_none());
            assert!(request.timeout.borrow().is_none());
        }
        while context.pending() {
            context.iteration(false);
        }
        assert!(closed.load(Ordering::SeqCst) > 0);
        bus.unregister_object(registration).unwrap();
    }
}
