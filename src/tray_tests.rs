use super::*;
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

const WATCHER: &str = "org.kde.StatusNotifierWatcher";
const ITEM: &str = "org.kde.StatusNotifierItem";

struct Watcher {
    items: Arc<Mutex<Vec<String>>>,
    main_loop: glib::MainLoop,
    thread: Option<thread::JoinHandle<()>>,
}

impl Watcher {
    fn start() -> Self {
        let items = Arc::new(Mutex::new(Vec::<String>::new()));
        let registered = items.clone();
        let (ready, receiver) = mpsc::channel();
        let thread = thread::spawn(move || {
            let context = glib::MainContext::new();
            context.with_thread_default(|| {
                let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
                let node = gio::DBusNodeInfo::for_xml(r#"
                    <node><interface name="org.kde.StatusNotifierWatcher">
                      <method name="RegisterStatusNotifierItem"><arg type="s" direction="in"/></method>
                      <property name="RegisteredStatusNotifierItems" type="as" access="read"/>
                      <property name="IsStatusNotifierHostRegistered" type="b" access="read"/>
                      <property name="ProtocolVersion" type="i" access="read"/>
                    </interface></node>"#).unwrap();
                let read_items = registered.clone();
                let registration = bus.register_object(
                    "/StatusNotifierWatcher", &node.interfaces()[0],
                    move |_, _, _, _, _, parameters, invocation| {
                        let (name,) = parameters.get::<(String,)>().unwrap();
                        registered.lock().unwrap().push(name);
                        invocation.return_value(None);
                    },
                    move |_, _, _, _, property| match property {
                        "RegisteredStatusNotifierItems" => read_items.lock().unwrap().to_variant(),
                        "IsStatusNotifierHostRegistered" => true.to_variant(),
                        "ProtocolVersion" => 0i32.to_variant(),
                        _ => unreachable!(),
                    },
                    |_, _, _, _, _, _| false,
                ).unwrap();
                let reply = bus.call_sync(
                    Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "RequestName",
                    Some(&(WATCHER, 4u32).to_variant()), None, gio::DBusCallFlags::NONE, 1000, gio::Cancellable::NONE,
                ).unwrap();
                assert_eq!(reply.get::<(u32,)>(), Some((1,)), "Use a private test bus, never a real desktop tray");
                let main_loop = glib::MainLoop::new(Some(&context), false);
                ready.send(main_loop.clone()).unwrap();
                main_loop.run();
                bus.call_sync(
                    Some("org.freedesktop.DBus"), "/org/freedesktop/DBus", "org.freedesktop.DBus", "ReleaseName",
                    Some(&(WATCHER,).to_variant()), None, gio::DBusCallFlags::NONE, 1000, gio::Cancellable::NONE,
                ).unwrap();
                bus.unregister_object(registration).unwrap();
            }).unwrap();
        });
        Self {
            items,
            main_loop: receiver.recv_timeout(Duration::from_secs(5)).unwrap(),
            thread: Some(thread),
        }
    }
}

impl Drop for Watcher {
    fn drop(&mut self) {
        self.main_loop.quit();
        self.thread.take().unwrap().join().unwrap();
    }
}

fn wait_until(check: impl Fn() -> bool) {
    let context = glib::MainContext::default();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        while context.pending() {
            context.iteration(false);
        }
        if check() {
            return;
        }
        assert!(Instant::now() < deadline, "Tray interaction timed out");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "requires a private Xvfb display, D-Bus session and XDG config/data directories"]
fn tray_dbus_activation() {
    assert!(std::env::var_os("XDG_CONFIG_HOME").is_some());
    assert!(std::env::var_os("XDG_DATA_HOME").is_some());
    gtk::init().unwrap();
    let watcher = Watcher::start();
    let app = gtk::Application::new(
        Some("io.github.opencolorpicker.TrayTest"),
        gio::ApplicationFlags::NON_UNIQUE,
    );
    app.register(gio::Cancellable::NONE).unwrap();
    let ui = Window::new(&app);
    ui.present();
    wait_until(|| ui.has_tray() && !watcher.items.lock().unwrap().is_empty());
    let name = watcher.items.lock().unwrap()[0].clone();
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
    let call = |path: &str, interface: &str, method: &str, args: glib::Variant| {
        bus.call_sync(
            Some(&name),
            path,
            interface,
            method,
            Some(&args),
            None,
            gio::DBusCallFlags::NONE,
            2000,
            gio::Cancellable::NONE,
        )
        .unwrap()
    };
    let property = |name: &str| {
        call(
            "/StatusNotifierItem",
            "org.freedesktop.DBus.Properties",
            "Get",
            (ITEM, name).to_variant(),
        )
        .child_value(0)
        .as_variant()
        .unwrap()
    };
    assert_eq!(property("ItemIsMenu").get::<bool>(), Some(false));
    let menu_path = property("Menu").str().unwrap().to_owned();

    // A right click asks the host to display the exported DBusMenu. Merely
    // opening it must not trigger picking or bring the main window forward.
    ui.hide_to_tray();
    assert!(!ui.widget.is_visible());
    let reply = call(
        &menu_path,
        "com.canonical.dbusmenu",
        "GetLayout",
        (0i32, -1i32, Vec::<String>::new()).to_variant(),
    );
    type Layout = (i32, HashMap<String, glib::Variant>, Vec<glib::Variant>);
    let (_, root) = reply.get::<(u32, Layout)>().unwrap();
    let entries: HashMap<String, i32> = root
        .2
        .iter()
        .filter_map(|child| {
            let (id, properties, _) = child.get::<Layout>()?;
            Some((properties.get("label")?.get::<String>()?, id))
        })
        .collect();
    assert!(entries.contains_key(tr(Text::PickColor)));
    assert!(entries.contains_key(tr(Text::ShowColors)));
    assert!(entries.contains_key(tr(Text::Quit)));
    assert_eq!(
        call(
            &menu_path,
            "com.canonical.dbusmenu",
            "AboutToShow",
            (0i32,).to_variant()
        )
        .get::<(bool,)>(),
        Some((false,))
    );
    assert!(!ui.busy.get());
    assert!(!ui.widget.is_visible());

    // Left click uses StatusNotifierItem.Activate, not a menu-item shortcut.
    call(
        "/StatusNotifierItem",
        ITEM,
        "Activate",
        (0i32, 0i32).to_variant(),
    );
    wait_until(|| ui.picker.borrow().is_some());
    let picker = ui.picker.borrow().as_ref().unwrap().clone();
    picker.close();
    wait_until(|| !ui.busy.get());
    let event = |label: &str| {
        call(
            &menu_path,
            "com.canonical.dbusmenu",
            "Event",
            (entries[label], "clicked", 0i32.to_variant(), 0u32).to_variant(),
        );
    };
    ui.hide_to_tray();
    event(tr(Text::ShowColors));
    wait_until(|| ui.widget.is_visible());
    event(tr(Text::PickColor));
    wait_until(|| ui.picker.borrow().is_some());
    ui.picker.borrow().as_ref().unwrap().clone().close();
    wait_until(|| !ui.busy.get());

    let locked = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let state = locked.clone();
    let node = gio::DBusNodeInfo::for_xml(
        r#"<node><interface name="org.gnome.ScreenSaver">
        <method name="GetActive"><arg type="b" direction="out"/></method>
        <signal name="ActiveChanged"><arg type="b"/></signal>
        </interface></node>"#,
    )
    .unwrap();
    let registration = bus
        .register_object(
            "/org/gnome/ScreenSaver",
            &node.interfaces()[0],
            move |_, _, _, _, _, _, invocation| {
                invocation.return_value(Some(
                    &(state.load(std::sync::atomic::Ordering::Relaxed),).to_variant(),
                ));
            },
            |_, _, _, _, _| unreachable!(),
            |_, _, _, _, _, _| false,
        )
        .unwrap();
    let owner = gio::bus_own_name_on_connection(
        &bus,
        "org.gnome.ScreenSaver",
        gio::BusNameOwnerFlags::NONE,
        |_, _| {},
        |_, _| {},
    );
    let set_locked = |active| {
        locked.store(active, std::sync::atomic::Ordering::Relaxed);
        bus.emit_signal(
            None,
            "/org/gnome/ScreenSaver",
            "org.gnome.ScreenSaver",
            "ActiveChanged",
            Some(&(active,).to_variant()),
        )
        .unwrap();
        wait_until(|| ui.session_lock.is_locked() == active);
    };
    let stays_hidden = || {
        let end = Instant::now() + Duration::from_millis(3500);
        wait_until(|| {
            assert!(
                !ui.widget.is_visible(),
                "Lock/unlock must preserve hidden windows"
            );
            Instant::now() >= end
        });
    };
    ui.hide_to_tray();
    drop(watcher);
    wait_until(|| !ui.has_tray());
    set_locked(true);
    stays_hidden();
    set_locked(false);
    let watcher = Watcher::start();
    wait_until(|| ui.has_tray() && !watcher.items.lock().unwrap().is_empty());
    stays_hidden();
    event(tr(Text::ShowColors));
    wait_until(|| ui.widget.is_visible());
    ui.hide_to_tray();
    drop(watcher);
    wait_until(|| !ui.has_tray() && ui.widget.is_visible());
    gio::bus_unown_name(owner);
    bus.unregister_object(registration).unwrap();
    let watcher = Watcher::start();
    wait_until(|| ui.has_tray() && !watcher.items.lock().unwrap().is_empty());
    assert_eq!(watcher.items.lock().unwrap()[0], name);

    let quit_called = Rc::new(Cell::new(false));
    app.lookup_action("quit")
        .unwrap()
        .downcast::<gio::SimpleAction>()
        .unwrap()
        .connect_activate(glib::clone!(@strong quit_called => move |_, _| quit_called.set(true)));
    event(tr(Text::Quit));
    wait_until(|| quit_called.get());
    // Explicitly release the item while its host is alive.
    ui.indicator.borrow_mut().take();
    assert!(!ui.has_tray());
    ui.widget.close();
}
