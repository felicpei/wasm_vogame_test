use std::{cell::RefCell, rc::Rc};
use std::{
    convert::TryInto,
    env,
};
use wayland_client::{
    protocol::{
        wl_keyboard::{KeymapFormat, WlKeyboard},
        wl_seat::WlSeat,
    },
    DispatchData, Main,
};
use xkb::x11::{MIN_MAJOR_XKB_VERSION, MIN_MINOR_XKB_VERSION};
use xkbcommon::xkb;

use xkbcommon::xkb::{KEYMAP_COMPILE_NO_FLAGS, KEYMAP_FORMAT_TEXT_V1};

use crate::errors::KeyLayoutError;

/// The KeyLayout struct
pub struct KeyLayout {
    keymap: xkb::Keymap,
}


impl KeyLayout {
    /// Construct a KeyLayout from a Winit window
    pub fn new_from_window(window: &winit::window::Window) -> Result<KeyLayout, KeyLayoutError> {
        if let Some(_conn) = winit::platform::unix::WindowExtUnix::xcb_connection(window) {
            Self::_new_x11()
        } else {
            Self::_new_wayland()
        }
    }

    /// Convert a scancode to a String
    pub fn get_key_as_string(&self, scancode: u32) -> String {
        let state = xkb::State::new(&self.keymap);

        // Offset of 8 between evdev scancodes and xkb scancodes

        // Get keysym from key
        let keysym = state.key_get_one_sym(scancode + 8);

        let mut output = state.key_get_utf8(scancode + 8);

        // Remove invisible characters
        output = output.replace(|c: char| c.is_control(), "");

        // Fallback to longer name if needed
        if output.trim().is_empty() {
            output = xkb::keysym_get_name(keysym);
        } else {
            output.make_ascii_uppercase();
        }

        output
    }

    fn _new_wayland() -> Result<Self, KeyLayoutError> {
        // We try Wayland

        let display = wayland_client::Display::connect_to_env().unwrap();

        // Set up the event queue
        let mut event_queue = display.create_event_queue();
        let token = event_queue.token();

        let proxy = &*display;
        let attached = proxy.attach(token);
        let registry = attached.get_registry();

        // Listen for available interfaces
        let available_interfaces = Rc::new(RefCell::new(Vec::<(u32, String, u32)>::new()));
        let available_interfaces_copy = Rc::clone(&available_interfaces);

        registry.quick_assign(move |_reg, event, _data| {
            if let wayland_client::protocol::wl_registry::Event::Global {
                name,
                interface,
                version,
            } = event
            {
                (*available_interfaces_copy)
                    .borrow_mut()
                    .push((name, interface, version));
            }
        });

        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .expect("Error during event queue roundtrip");

        // Bind to wl_seat if available
        // Find wl_seat tuple

        let (seat_name, _seat_interface, seat_version) = (*available_interfaces)
            .borrow()
            .iter()
            .find(|(_name, interface, _version)| interface == "wl_seat")
            .expect("wl_seat not found in available interfaces")
            .clone();

        attached.sync();

        let wl_seat = registry.bind::<WlSeat>(seat_version, seat_name);

        let capabilities = Rc::new(RefCell::new(
            wayland_client::protocol::wl_seat::Capability::empty(),
        ));
        let capabilities_copy = Rc::clone(&capabilities);
        wl_seat.quick_assign(move |_seat, event, _data| {
            if let wayland_client::protocol::wl_seat::Event::Capabilities { capabilities } = event {
                (*capabilities_copy).borrow_mut().set(capabilities, true);
            }
        });

        event_queue
            .sync_roundtrip(&mut (), |_, _, _| {})
            .expect("Error during event queue roundtrip");

        // Check capabilities of wl_seat
        if !(*capabilities)
            .borrow()
            .contains(wayland_client::protocol::wl_seat::Capability::Keyboard)
        {
            panic!("wl_seat does not have keyboard capability");
        }

        let wl_keyboard = wl_seat.get_keyboard();

        let file_descriptor = Rc::new(RefCell::new(-1));
        let size = Rc::new(RefCell::new(0));

        let file_descriptor_copy = Rc::clone(&file_descriptor);
        let size_copy = Rc::clone(&size);

        // Get keymap from compositor

        wl_keyboard.quick_assign(
            move |_object: Main<WlKeyboard>,
                  event: wayland_client::protocol::wl_keyboard::Event,
                  _data: DispatchData<'_>| {
                if let wayland_client::protocol::wl_keyboard::Event::Keymap { format, fd, size } =
                    event
                {
                    match format {
                        KeymapFormat::XkbV1 => {
                            *file_descriptor_copy.borrow_mut() = fd;
                            *size_copy.borrow_mut() = size;
                        }
                        KeymapFormat::NoKeymap => {
                            panic!("NoKeymap format");
                        }
                        _ => {
                            panic!("Keymap Format not supported");
                        }
                    };
                }
            },
        );

        event_queue.sync_roundtrip(&mut (), |_, _, _| {}).unwrap();

        // Construct keymap from file descriptor

        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        let keymap = xkb::Keymap::new_from_fd(
            &ctx,
            *file_descriptor.borrow(),
            (*size.borrow()).try_into().unwrap(),
            KEYMAP_FORMAT_TEXT_V1,
            KEYMAP_COMPILE_NO_FLAGS,
        )
        .expect("Failed to create keymap.");

        Ok(Self { keymap })
    }

    fn _new_x11() -> Result<Self, KeyLayoutError> {
        let (conn, _) = xcb::base::Connection::connect(None).unwrap();
        let mut major_xkb_version_out = 0;
        let mut minor_xkb_version_out = 0;
        let mut base_event_out = 0;
        let mut base_error_out = 0;

        let _ = xkb::x11::setup_xkb_extension(
            &conn,
            MIN_MAJOR_XKB_VERSION,
            MIN_MINOR_XKB_VERSION,
            xkb::x11::SetupXkbExtensionFlags::NoFlags,
            &mut major_xkb_version_out,
            &mut minor_xkb_version_out,
            &mut base_event_out,
            &mut base_error_out,
        );

        let device_id = xkb::x11::get_core_keyboard_device_id(&conn);

        let ctx = xkb::Context::new(xkb::CONTEXT_NO_FLAGS);

        let keymap = xkb::x11::keymap_new_from_device(&ctx, &conn, device_id, 0);

        Ok(KeyLayout { keymap })
    }

    /// Construct a KeyLayout
    /// Tries to autodetect the session type using the XDG_SESSION_TYPE environment variable
    pub fn new() -> Result<KeyLayout, KeyLayoutError> {
        match env::var("XDG_SESSION_TYPE") {
            Ok(session_type) => match session_type.as_str() {
                "wayland" => Self::_new_wayland(),
                "x11" => Self::_new_x11(),
                _ => Err(KeyLayoutError::SessionError),
            },
            Err(_e) => Err(KeyLayoutError::SessionError),
        }
    }
}

/// Methods for KeyLayout specific to Unix-based systems
#[cfg(target_os = "linux")]
pub trait KeyLayoutExtUnix {

    /// Construct a KeyLayout explicitly using the Wayland protocol
    fn new_wayland () -> Result<KeyLayout, KeyLayoutError>;

    /// Construct a KeyLayout explicitly using the X11 protocol
    fn new_x11() -> Result<KeyLayout, KeyLayoutError>;

}

impl KeyLayoutExtUnix for KeyLayout {

    fn new_wayland() -> Result<KeyLayout, KeyLayoutError> {
        Self::_new_wayland()
    }

    fn new_x11() -> Result<KeyLayout, KeyLayoutError> {
        Self::_new_x11()
    }

}