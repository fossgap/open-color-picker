//! Live desktop frames from the ScreenCast portal. All capture sessions are
//! closed when picking ends. Unsupported backends fall back to a still capture.
use crate::pixels::MonitorArea;
use gstreamer::{self as gst, prelude::*};
use gtk::{
    gdk,
    gdk_pixbuf::{Colorspace, InterpType, Pixbuf},
    gio, glib,
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    os::fd::{AsRawFd, FromRawFd, OwnedFd},
    rc::Rc,
    time::{Duration, Instant},
};

const DEST: &str = "org.freedesktop.portal.Desktop";
const PATH: &str = "/org/freedesktop/portal/desktop";
const INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
type Options = HashMap<String, glib::Variant>;
type Callback = Box<dyn FnOnce(Outcome)>;

pub enum Outcome {
    Ready(Pixbuf, Stream),
    Unavailable,
    Cancelled,
}

#[derive(Debug)]
enum Error {
    Cancelled,
    Unavailable(String),
}
type Result<T> = std::result::Result<T, Error>;
fn unavailable(error: impl std::fmt::Display) -> Error {
    Error::Unavailable(error.to_string())
}

pub struct Request {
    bus: gio::DBusConnection,
    session: RefCell<Option<Lease>>,
    task: RefCell<Option<glib::JoinHandle<()>>>,
    callback: RefCell<Option<Callback>>,
}

pub fn start(callback: impl FnOnce(Outcome) + 'static) -> Option<Rc<Request>> {
    // xdg_toplevel.set_fullscreen explicitly hides other surfaces behind a
    // transparent fullscreen window. On Wayland this overlay would therefore
    // show (and capture) a black backdrop. Fall back before requesting sharing.
    if crate::capture::is_wayland() {
        callback(Outcome::Unavailable);
        return None;
    }
    let supported = gdk::Screen::default()
        .is_some_and(|s| s.is_composited() && s.rgba_visual().is_some())
        && gst::init().is_ok()
        && ["pipewiresrc", "videoconvert", "appsink"]
            .iter()
            .all(|name| gst::ElementFactory::find(name).is_some());
    if !supported {
        callback(Outcome::Unavailable);
        return None;
    }
    let Ok(bus) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        callback(Outcome::Unavailable);
        return None;
    };
    Some(Request::start(bus, Box::new(callback)))
}

impl Request {
    fn start(bus: gio::DBusConnection, callback: Callback) -> Rc<Self> {
        let request = Rc::new(Self {
            bus,
            session: RefCell::new(None),
            task: RefCell::new(None),
            callback: RefCell::new(Some(callback)),
        });
        let cloned = request.clone();
        *request.task.borrow_mut() = Some(glib::MainContext::default().spawn_local(async move {
            let result = cloned.capture().await;
            cloned.task.borrow_mut().take();
            let outcome = match result {
                Ok((image, stream)) => Outcome::Ready(image, stream),
                Err(Error::Cancelled) => Outcome::Cancelled,
                Err(Error::Unavailable(error)) => {
                    eprintln!("Live desktop capture unavailable: {error}. Using a frozen overlay.");
                    Outcome::Unavailable
                }
            };
            cloned.close();
            let callback = cloned.callback.borrow_mut().take();
            if let Some(callback) = callback {
                callback(outcome);
            }
        }));
        request
    }
    pub fn cancel(&self) {
        if let Some(task) = self.task.borrow_mut().take() {
            task.abort();
        }
        self.close();
        let callback = self.callback.borrow_mut().take();
        if let Some(callback) = callback {
            callback(Outcome::Cancelled);
        }
    }

    fn close(&self) {
        self.session.borrow_mut().take();
    }

    async fn response(
        &self,
        method: &str,
        prefix: Option<&str>,
        mut options: Options,
    ) -> Result<Options> {
        let token = format!("ocp_live_{}", glib::uuid_string_random().replace('-', "_"));
        let sender = self
            .bus
            .unique_name()
            .unwrap()
            .trim_start_matches(':')
            .replace('.', "_");
        let path = format!("{PATH}/request/{sender}/{token}");
        options.insert("handle_token".into(), token.to_variant());
        let parameters = match (method, prefix) {
            ("Start", Some(session)) => (
                glib::variant::ObjectPath::try_from(session).map_err(unavailable)?,
                "",
                options,
            )
                .to_variant(),
            (_, Some(session)) => (
                glib::variant::ObjectPath::try_from(session).map_err(unavailable)?,
                options,
            )
                .to_variant(),
            _ => (options,).to_variant(),
        };
        let (send, receive) = futures_channel::oneshot::channel();
        let send = Rc::new(RefCell::new(Some(send)));
        let subscribe = |path: &str| {
            let send = send.clone();
            self.bus.signal_subscribe(
                Some(DEST),
                Some("org.freedesktop.portal.Request"),
                Some("Response"),
                Some(path),
                None,
                gio::DBusSignalFlags::NONE,
                move |_, _, _, _, _, response| {
                    let result = match response.get::<(u32, Options)>() {
                        Some((0, results)) => Ok(results),
                        Some((1, _)) => Err(Error::Cancelled),
                        _ => Err(unavailable("The desktop rejected the live capture request")),
                    };
                    if let Some(send) = send.borrow_mut().take() {
                        let _ = send.send(result);
                    }
                },
            )
        };
        let expired = Rc::new(Cell::new(false));
        let expired_flag = expired.clone();
        let timeout_sender = send.clone();
        let timeout = glib::timeout_add_local_once(Duration::from_secs(120), move || {
            expired_flag.set(true);
            if let Some(send) = timeout_sender.borrow_mut().take() {
                let _ = send.send(Err(unavailable("Live capture timed out")));
            }
        });
        let mut pending = Pending {
            bus: self.bus.clone(),
            path: path.clone(),
            subscription: Some(subscribe(&path)),
            timeout: Some(timeout),
            expired,
        };
        let reply = self
            .bus
            .call_future(
                Some(DEST),
                PATH,
                INTERFACE,
                method,
                Some(&parameters),
                None,
                gio::DBusCallFlags::NONE,
                120_000,
            )
            .await
            .map_err(unavailable)?;
        let (actual,) = reply
            .get::<(glib::variant::ObjectPath,)>()
            .ok_or_else(|| unavailable("Invalid portal handle"))?;
        if actual.as_str() != path {
            if let Some(id) = pending.subscription.take() {
                self.bus.signal_unsubscribe(id);
            }
            pending.path = actual.to_string();
            pending.subscription = Some(subscribe(&pending.path));
        }
        receive.await.map_err(unavailable)?
    }

    async fn capture(&self) -> Result<(Pixbuf, Stream)> {
        let token = format!(
            "ocp_session_{}",
            glib::uuid_string_random().replace('-', "_")
        );
        let sender = self
            .bus
            .unique_name()
            .unwrap()
            .trim_start_matches(':')
            .replace('.', "_");
        // Track the predicted session before the reply, so cancellation also
        // closes a session created while CreateSession was in flight.
        *self.session.borrow_mut() = Some(Lease::new(
            &self.bus,
            format!("{PATH}/session/{sender}/{token}"),
        ));
        let created = self
            .response(
                "CreateSession",
                None,
                HashMap::from([("session_handle_token".into(), token.to_variant())]),
            )
            .await?;
        let session = created
            .get("session_handle")
            .and_then(|v| v.get::<String>())
            .ok_or_else(|| unavailable("No capture session"))?;
        let object = glib::variant::ObjectPath::try_from(session.as_str()).map_err(unavailable)?;
        if self.session.borrow().as_ref().unwrap().path != session {
            *self.session.borrow_mut() = Some(Lease::new(&self.bus, session.clone()));
        }
        self.response(
            "SelectSources",
            Some(&session),
            HashMap::from([
                ("types".into(), 1u32.to_variant()),
                ("multiple".into(), true.to_variant()),
                ("cursor_mode".into(), 1u32.to_variant()),
            ]),
        )
        .await?;
        let started = self
            .response("Start", Some(&session), HashMap::new())
            .await?;
        let streams = started
            .get("streams")
            .and_then(|v| v.get::<Vec<(u32, Options)>>())
            .ok_or_else(|| unavailable("No desktop streams"))?;
        let display = gdk::Display::default().ok_or_else(|| unavailable("No display"))?;
        let monitors: Vec<_> = (0..display.n_monitors())
            .filter_map(|i| display.monitor(i).map(|m| m.geometry()))
            .collect();
        let mut sources = Vec::new();
        for (node, properties) in streams {
            let geometry = match (
                properties
                    .get("position")
                    .and_then(|v| v.get::<(i32, i32)>()),
                properties.get("size").and_then(|v| v.get::<(i32, i32)>()),
            ) {
                (Some((x, y)), Some((w, h))) => monitors
                    .iter()
                    .find(|m| m.x() == x && m.y() == y && m.width() == w && m.height() == h)
                    .copied(),
                _ if monitors.len() == 1 => monitors.first().copied(),
                _ => None,
            }
            .ok_or_else(|| unavailable("The stream does not identify a desktop monitor"))?;
            sources.push((
                node,
                MonitorArea {
                    x: geometry.x(),
                    y: geometry.y(),
                    width: geometry.width(),
                    height: geometry.height(),
                },
            ));
        }
        if sources.is_empty() {
            return Err(unavailable("No monitors were selected"));
        }
        let parameters = (object, HashMap::<String, glib::Variant>::new()).to_variant();
        let (reply, fds) = self
            .bus
            .call_with_unix_fd_list_future(
                Some(DEST),
                PATH,
                INTERFACE,
                "OpenPipeWireRemote",
                Some(&parameters),
                None,
                gio::DBusCallFlags::NONE,
                10_000,
                None::<&gio::UnixFDList>,
            )
            .await
            .map_err(unavailable)?;
        let (handle,) = reply
            .get::<(glib::variant::Handle,)>()
            .ok_or_else(|| unavailable("No PipeWire connection"))?;
        let fd = fds.get(handle.0).map_err(unavailable)?;
        // GUnixFDList::get returns a new descriptor owned by its caller.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let lease = self.session.borrow_mut().take().unwrap();
        let mut stream = Stream {
            pipes: Vec::new(),
            areas: sources.iter().map(|(_, area)| *area).collect(),
            remote: Some(fd),
            lease: Some(lease),
            dimensions: None,
        };
        for (node, _) in sources {
            let source = gst::ElementFactory::make("pipewiresrc")
                .property("fd", stream.remote.as_ref().unwrap().as_raw_fd())
                .property("path", node.to_string())
                .property("do-timestamp", true)
                .build()
                .map_err(unavailable)?;
            stream.pipes.push(Pipe::new(source).map_err(unavailable)?);
        }
        loop {
            if let Some(image) = stream.poll().map_err(unavailable)? {
                return Ok((image, stream));
            }
            glib::timeout_future(Duration::from_millis(40)).await;
        }
    }
}

fn close(bus: &gio::DBusConnection, path: &str, interface: &str) {
    bus.call(
        Some(DEST),
        path,
        interface,
        "Close",
        None,
        None,
        gio::DBusCallFlags::NONE,
        1000,
        gio::Cancellable::NONE,
        |_| {},
    );
}

struct Pending {
    bus: gio::DBusConnection,
    path: String,
    subscription: Option<gio::SignalSubscriptionId>,
    timeout: Option<glib::SourceId>,
    expired: Rc<Cell<bool>>,
}
impl Drop for Pending {
    fn drop(&mut self) {
        if let Some(id) = self.subscription.take() {
            self.bus.signal_unsubscribe(id);
        }
        if !self.expired.get() {
            if let Some(timeout) = self.timeout.take() {
                timeout.remove();
            }
        }
        close(&self.bus, &self.path, "org.freedesktop.portal.Request");
    }
}
struct Lease {
    bus: gio::DBusConnection,
    path: String,
    closed: Rc<Cell<bool>>,
    subscription: Option<gio::SignalSubscriptionId>,
}
impl Lease {
    fn new(bus: &gio::DBusConnection, path: String) -> Self {
        let closed = Rc::new(Cell::new(false));
        let flag = closed.clone();
        let subscription = bus.signal_subscribe(
            Some(DEST),
            Some("org.freedesktop.portal.Session"),
            Some("Closed"),
            Some(&path),
            None,
            gio::DBusSignalFlags::NONE,
            move |_, _, _, _, _, _| flag.set(true),
        );
        Self {
            bus: bus.clone(),
            path,
            closed,
            subscription: Some(subscription),
        }
    }
}
impl Drop for Lease {
    fn drop(&mut self) {
        if let Some(id) = self.subscription.take() {
            self.bus.signal_unsubscribe(id);
        }
        if !self.closed.get() {
            close(&self.bus, &self.path, "org.freedesktop.portal.Session");
        }
    }
}

struct Pipe {
    pipeline: gst::Pipeline,
    sink: gstreamer_app::AppSink,
    frame: Option<Pixbuf>,
    last_frame: Instant,
}
impl Pipe {
    fn new(source: gst::Element) -> std::result::Result<Self, String> {
        let pipeline = gst::Pipeline::new();
        let convert = gst::ElementFactory::make("videoconvert")
            .build()
            .map_err(|e| e.to_string())?;
        let sink = gstreamer_app::AppSink::builder()
            .caps(
                &gst::Caps::builder("video/x-raw")
                    .field("format", "RGB")
                    .build(),
            )
            .max_buffers(1)
            .drop(true)
            .sync(false)
            .build();
        pipeline
            .add_many([&source, &convert, sink.upcast_ref()])
            .map_err(|e| e.to_string())?;
        gst::Element::link_many([&source, &convert, sink.upcast_ref()])
            .map_err(|e| e.to_string())?;
        let pipe = Self {
            pipeline,
            sink,
            frame: None,
            last_frame: Instant::now(),
        };
        pipe.pipeline
            .set_state(gst::State::Playing)
            .map_err(|e| e.to_string())?;
        Ok(pipe)
    }

    fn update(&mut self) -> std::result::Result<bool, String> {
        if let Some(message) = self
            .pipeline
            .bus()
            .unwrap()
            .pop_filtered(&[gst::MessageType::Error, gst::MessageType::Eos])
        {
            return Err(format!("Capture stream stopped: {:?}", message.view()));
        }
        if let Some(sample) = self.sink.try_pull_sample(gst::ClockTime::ZERO) {
            let info =
                gstreamer_video::VideoInfo::from_caps(sample.caps().ok_or("Missing video format")?)
                    .map_err(|e| e.to_string())?;
            let frame = gstreamer_video::VideoFrameRef::from_buffer_ref_readable(
                sample.buffer().ok_or("Missing video frame")?,
                &info,
            )
            .map_err(|e| e.to_string())?;
            let plane = frame.plane_data(0).map_err(|e| e.to_string())?;
            let stride = usize::try_from(frame.plane_stride()[0]).map_err(|e| e.to_string())?;
            let row = info.width() as usize * 3;
            if stride < row {
                return Err("Invalid video stride".into());
            }
            let mut rgb = Vec::with_capacity(row * info.height() as usize);
            for y in 0..info.height() as usize {
                rgb.extend_from_slice(
                    plane
                        .get(y * stride..y * stride + row)
                        .ok_or("Incomplete frame")?,
                );
            }
            self.frame = Some(Pixbuf::from_bytes(
                &glib::Bytes::from_owned(rgb),
                Colorspace::Rgb,
                false,
                8,
                info.width() as i32,
                info.height() as i32,
                row as i32,
            ));
            self.last_frame = Instant::now();
            Ok(true)
        } else if self.frame.is_none() && self.last_frame.elapsed() > Duration::from_secs(3) {
            Err("Desktop frames stopped arriving".into())
        } else {
            Ok(false)
        }
    }
}
impl Drop for Pipe {
    fn drop(&mut self) {
        let _ = self.pipeline.set_state(gst::State::Null);
    }
}

pub struct Stream {
    pipes: Vec<Pipe>,
    pub areas: Vec<MonitorArea>,
    remote: Option<OwnedFd>,
    lease: Option<Lease>,
    dimensions: Option<(i32, i32)>,
}
impl Stream {
    pub fn poll(&mut self) -> std::result::Result<Option<Pixbuf>, String> {
        if self.lease.as_ref().is_some_and(|lease| lease.closed.get()) {
            return Err("Desktop sharing ended".into());
        }
        let mut changed = false;
        for pipe in &mut self.pipes {
            changed |= pipe.update()?;
        }
        if !changed || self.pipes.iter().any(|p| p.frame.is_none()) {
            return Ok(None);
        }
        let left = self.areas.iter().map(|a| a.x).min().unwrap();
        let top = self.areas.iter().map(|a| a.y).min().unwrap();
        let right = self.areas.iter().map(|a| a.x + a.width).max().unwrap();
        let bottom = self.areas.iter().map(|a| a.y + a.height).max().unwrap();
        let scale = self
            .pipes
            .iter()
            .zip(&self.areas)
            .map(|(p, a)| f64::from(p.frame.as_ref().unwrap().width()) / f64::from(a.width))
            .fold(0.0, f64::max);
        let width = (f64::from(right - left) * scale).round() as i32;
        let height = (f64::from(bottom - top) * scale).round() as i32;
        if self.dimensions.is_some_and(|size| size != (width, height)) {
            return Err("Desktop resolution changed".into());
        }
        self.dimensions = Some((width, height));
        if self.pipes.len() == 1 {
            return Ok(self.pipes[0].frame.clone());
        }
        let image = Pixbuf::new(Colorspace::Rgb, false, 8, width, height)
            .ok_or("Could not allocate desktop frame")?;
        image.fill(0x000000ff);
        for (pipe, area) in self.pipes.iter().zip(&self.areas) {
            let frame = pipe.frame.as_ref().unwrap();
            let w = (f64::from(area.width) * scale).round() as i32;
            let h = (f64::from(area.height) * scale).round() as i32;
            let frame = if (frame.width(), frame.height()) == (w, h) {
                frame.clone()
            } else {
                frame
                    .scale_simple(w, h, InterpType::Nearest)
                    .ok_or("Could not map monitor frame")?
            };
            frame.copy_area(
                0,
                0,
                w,
                h,
                &image,
                (f64::from(area.x - left) * scale).round() as i32,
                (f64::from(area.y - top) * scale).round() as i32,
            );
        }
        Ok(Some(image))
    }
}

#[cfg(test)]
pub(crate) fn test_desktop_stream(area: MonitorArea) -> Stream {
    gst::init().unwrap();
    let source = gst::ElementFactory::make("ximagesrc")
        .property("use-damage", false)
        .property("show-pointer", false)
        .build()
        .unwrap();
    Stream {
        pipes: vec![Pipe::new(source).unwrap()],
        areas: vec![area],
        remote: None,
        lease: None,
        dimensions: None,
    }
}

#[cfg(test)]
pub(crate) fn test_stream(area: MonitorArea) -> (Stream, gst::Element, gst::Pipeline) {
    gst::init().unwrap();
    let bin = gst::parse_bin_from_description(
        &format!("videotestsrc name=desktop is-live=true pattern=red ! capsfilter caps=\"video/x-raw,width={},height={},framerate=25/1\"", area.width, area.height), true,
    ).unwrap();
    let source = bin.by_name("desktop").unwrap();
    let pipe = Pipe::new(bin.upcast()).unwrap();
    let pipeline = pipe.pipeline.clone();
    (
        Stream {
            pipes: vec![pipe],
            areas: vec![area],
            remote: None,
            lease: None,
            dimensions: None,
        },
        source,
        pipeline,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{color::Color, pixels::Pixels};

    #[test]
    fn live_frames_change_and_stream_shutdown_is_detected() {
        let (mut stream, source, pipeline) = test_stream(MonitorArea {
            x: 0,
            y: 0,
            width: 64,
            height: 48,
        });
        for (pattern, expected) in [("red", Color(255, 0, 0)), ("green", Color(0, 255, 0))] {
            source.set_property_from_str("pattern", pattern);
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                if let Some(frame) = stream.poll().unwrap() {
                    assert_eq!((frame.width(), frame.height()), (64, 48));
                    if Pixels::new(&frame).at(32, 24) == expected {
                        break;
                    }
                }
                assert!(Instant::now() < deadline, "No {pattern} frame arrived");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        assert!(pipeline.send_event(gst::event::Eos::new()));
        let deadline = Instant::now() + Duration::from_secs(2);
        while stream.poll().is_ok() {
            assert!(
                Instant::now() < deadline,
                "Stream shutdown was not detected"
            );
            std::thread::sleep(Duration::from_millis(10));
        }
        drop(stream);
        assert_eq!(pipeline.current_state(), gst::State::Null);
    }

    #[test]
    #[ignore = "requires private Xvfb and D-Bus; run with scripts/check.sh"]
    fn live_portal_cancellation_and_fallback() {
        use std::sync::{
            atomic::{AtomicU32, Ordering},
            Arc, Mutex,
        };
        gtk::init().unwrap();
        let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).unwrap();
        let reply = bus
            .call_sync(
                Some("org.freedesktop.DBus"),
                "/org/freedesktop/DBus",
                "org.freedesktop.DBus",
                "RequestName",
                Some(&(DEST, 4u32).to_variant()),
                None,
                gio::DBusCallFlags::NONE,
                1000,
                gio::Cancellable::NONE,
            )
            .unwrap();
        assert_eq!(
            reply.get::<(u32,)>(),
            Some((1,)),
            "Use a private D-Bus session"
        );
        let node = gio::DBusNodeInfo::for_xml(r#"<node><interface name="org.freedesktop.portal.ScreenCast">
          <method name="CreateSession"><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <method name="SelectSources"><arg type="o" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
          <method name="Start"><arg type="o" direction="in"/><arg type="s" direction="in"/><arg type="a{sv}" direction="in"/><arg type="o" direction="out"/></method>
        </interface></node>"#).unwrap();
        let mode = Arc::new(AtomicU32::new(0));
        let calls = Arc::new(Mutex::new(Vec::new()));
        let session = format!("{PATH}/session/test/capture");
        let closed = Arc::new(AtomicU32::new(0));
        let close_node = gio::DBusNodeInfo::for_xml(r#"<node><interface name="org.freedesktop.portal.Session"><method name="Close"/></interface></node>"#).unwrap();
        let close_count = closed.clone();
        let session_registration = bus
            .register_object(
                &session,
                &close_node.interfaces()[0],
                move |_, _, _, _, _, _, invocation| {
                    close_count.fetch_add(1, Ordering::SeqCst);
                    invocation.return_value(Some(&().to_variant()));
                },
                |_, _, _, _, _| ().to_variant(),
                |_, _, _, _, _, _| false,
            )
            .unwrap();
        let scenario = mode.clone();
        let observed = calls.clone();
        let registration = bus
            .register_object(
                PATH,
                &node.interfaces()[0],
                move |connection, sender, _, _, method, parameters, invocation| {
                    observed.lock().unwrap().push(method.to_owned());
                    let options = parameters
                        .child_value(parameters.n_children() - 1)
                        .get::<Options>()
                        .unwrap();
                    let token = options["handle_token"].get::<String>().unwrap();
                    let path = format!(
                        "{PATH}/request/{}/{token}",
                        sender.trim_start_matches(':').replace('.', "_")
                    );
                    invocation.return_value(Some(
                        &(glib::variant::ObjectPath::try_from(path.as_str()).unwrap(),)
                            .to_variant(),
                    ));
                    if method == "SelectSources" {
                        assert_eq!(options["types"].get::<u32>(), Some(1));
                        assert_eq!(options["cursor_mode"].get::<u32>(), Some(1));
                    }
                    if method == "Start" && scenario.load(Ordering::SeqCst) == 2 {
                        return;
                    } // Cancel an in-flight request.
                    let response = if method == "Start" && scenario.load(Ordering::SeqCst) == 1 {
                        1u32
                    } else {
                        0
                    };
                    let results: Options = if method == "CreateSession" {
                        HashMap::from([("session_handle".into(), session.to_variant())])
                    } else {
                        HashMap::new()
                    }; // Missing streams must trigger frozen fallback.
                    connection
                        .emit_signal(
                            Some(sender),
                            &path,
                            "org.freedesktop.portal.Request",
                            "Response",
                            Some(&(response, results).to_variant()),
                        )
                        .unwrap();
                },
                |_, _, _, _, _| ().to_variant(),
                |_, _, _, _, _, _| false,
            )
            .unwrap();
        let context = glib::MainContext::default();
        let spin_until = |ready: &dyn Fn() -> bool| {
            let deadline = Instant::now() + Duration::from_secs(3);
            while !ready() {
                assert!(Instant::now() < deadline, "Portal operation did not finish");
                while context.pending() {
                    context.iteration(false);
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        };
        for scenario in 0..3 {
            mode.store(scenario, Ordering::SeqCst);
            calls.lock().unwrap().clear();
            let result = Rc::new(RefCell::new(Vec::new()));
            let delivered = result.clone();
            let request = Request::start(
                bus.clone(),
                Box::new(move |outcome| {
                    delivered.borrow_mut().push(match outcome {
                        Outcome::Unavailable => 0,
                        Outcome::Cancelled => 1,
                        Outcome::Ready(..) => 2,
                    });
                }),
            );
            if scenario == 2 {
                spin_until(&|| {
                    calls
                        .lock()
                        .unwrap()
                        .last()
                        .is_some_and(|method| method == "Start")
                });
                request.cancel();
            }
            spin_until(&|| {
                !result.borrow().is_empty() && closed.load(Ordering::SeqCst) == scenario + 1
            });
            request.cancel(); // Callback and cleanup must happen only once.
            assert_eq!(*result.borrow(), vec![u32::from(scenario != 0)]);
            assert_eq!(
                *calls.lock().unwrap(),
                vec!["CreateSession", "SelectSources", "Start"]
            );
            assert!(request.session.borrow().is_none());
            assert!(request.task.borrow().is_none());
        }
        // A compositor ending sharing must freeze even if no EOS frame arrives.
        let (mut stream, _, _) = test_stream(MonitorArea {
            x: 0,
            y: 0,
            width: 64,
            height: 48,
        });
        let session = format!("{PATH}/session/test/closed");
        stream.lease = Some(Lease::new(&bus, session.clone()));
        bus.emit_signal(
            None,
            &session,
            "org.freedesktop.portal.Session",
            "Closed",
            Some(&(HashMap::<String, glib::Variant>::new(),).to_variant()),
        )
        .unwrap();
        spin_until(&|| stream.lease.as_ref().unwrap().closed.get());
        assert!(stream.poll().is_err());
        bus.unregister_object(registration).unwrap();
        bus.unregister_object(session_registration).unwrap();
    }
}
