use std::{ops::Deref, rc::Rc};

use raqote::DrawTarget;
use x11rb::{
    connection::Connection as _,
    properties::WmSizeHints,
    protocol::{
        Event,
        xproto::{
            self, AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _,
            CreateWindowAux, EventMask, ImageFormat, KeyButMask, PropMode, StackMode, VisualClass,
            WindowClass,
        },
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
};

use crate::{CursorPos, Error, MouseButton, WindowEvent, error::err};

x11rb::atom_manager! {
    pub Atoms: AtomCookie {
        UTF8_STRING,

        WM_PROTOCOLS,
        WM_DELETE_WINDOW,

        _NET_WM_NAME,
        _NET_WM_WINDOW_TYPE,
        _NET_WM_WINDOW_TYPE_DIALOG,

        _NET_WM_MOVERESIZE,
    }
}

enum WindowType {
    Dialog,
}

#[derive(Clone)]
pub(crate) struct Connection {
    inner: Rc<RustConnection>,
    screen: usize,
}

impl Connection {
    pub(crate) fn connect() -> Result<Self, Error> {
        let (conn, screen) = x11rb::connect(None).map_err(err)?;
        Ok(Self {
            inner: Rc::new(conn),
            screen,
        })
    }
}

impl Deref for Connection {
    type Target = RustConnection;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

const MOVERESIZE_MOVE: u32 = 8;

const KEYCODE_ESC: u8 = 9;

const WM_CLASS: &[u8] = b"alerta\0alerta\0"; // instance, class

pub(crate) struct X11Window {
    atoms: Atoms,
    conn: Connection,
    window: xproto::Window,
    gc: xproto::Gcontext,
}

impl X11Window {
    pub(crate) fn create(conn: Connection, width: u16, height: u16) -> Result<Self, Error> {
        let atoms = Atoms::new(&conn.inner).map_err(err)?.reply().map_err(err)?;

        let screen = conn.inner.setup().roots.get(conn.screen).ok_or_else(|| {
            Error::new(format!("screen '{}' does not exist on server", conn.screen))
        })?;

        let visuals = screen
            .allowed_depths
            .iter()
            .flat_map(|d| d.visuals.iter().map(move |vis| (vis, d.depth)));
        let mut vid = None;
        for (vty, depth) in visuals {
            if depth == 24
                && vty.class == VisualClass::TRUE_COLOR
                && vty.red_mask == 0xff0000
                && vty.green_mask == 0xff00
                && vty.blue_mask == 0xff
            {
                vid = Some(vty.visual_id);
                break;
            }
        }

        let Some(vid) = vid else {
            return Err(Error::new("could not find a compatible X11 visual"));
        };

        let attrs = CreateWindowAux::new()
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::STRUCTURE_NOTIFY
                    | EventMask::VISIBILITY_CHANGE
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::PROPERTY_CHANGE
                    | EventMask::POINTER_MOTION
                    | EventMask::ENTER_WINDOW
                    | EventMask::LEAVE_WINDOW
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE,
            )
            .border_pixel(0)
            .colormap(0);

        let window = conn.generate_id().map_err(err)?;
        conn.create_window(
            24,
            window,
            screen.root,
            0,
            0,
            width,
            height,
            0,
            WindowClass::INPUT_OUTPUT,
            vid,
            &attrs,
        )
        .map_err(err)?
        .check()
        .map_err(err)?;

        let gc = conn.generate_id().map_err(err)?;
        conn.create_gc(
            gc,
            window,
            &xproto::CreateGCAux::new().graphics_exposures(0),
        )
        .map_err(err)?;

        // By default, X11 seems to kill the damn application with SIGTERM when
        // the window is closed (!?).
        // Opt into getting a `ClientMessage` event instead.
        conn.change_property32(
            PropMode::REPLACE,
            window,
            atoms.WM_PROTOCOLS,
            AtomEnum::ATOM,
            &[atoms.WM_DELETE_WINDOW],
        )
        .map_err(err)?;

        // Configure size hints to prevent resizing the window.
        WmSizeHints {
            max_size: Some((width.into(), height.into())),
            min_size: Some((width.into(), height.into())),
            ..Default::default()
        }
        .set_normal_hints(&conn.inner, window)
        .map_err(err)?
        .check()
        .map_err(err)?;

        let mut win = X11Window {
            atoms,
            conn,
            window,
            gc,
        };
        win = win
            .with_class(WM_CLASS)?
            .with_window_type(WindowType::Dialog)?;

        Ok(win)
    }

    fn with_class(self, cls: &[u8]) -> Result<Self, Error> {
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                AtomEnum::WM_CLASS,
                AtomEnum::STRING,
                cls,
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        Ok(self)
    }

    fn with_window_type(self, ty: WindowType) -> Result<Self, Error> {
        let atom = match ty {
            WindowType::Dialog => self.atoms._NET_WM_WINDOW_TYPE_DIALOG,
        };
        self.conn
            .change_property32(
                PropMode::REPLACE,
                self.window,
                self.atoms._NET_WM_WINDOW_TYPE,
                AtomEnum::ATOM,
                &[atom],
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        Ok(self)
    }

    pub(crate) fn with_title(self, mut title: String) -> Result<Self, Error> {
        if !title.ends_with('\0') {
            title.push('\0');
        }

        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                AtomEnum::WM_NAME,
                AtomEnum::STRING,
                title.as_bytes(),
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        self.conn
            .change_property8(
                PropMode::REPLACE,
                self.window,
                self.atoms._NET_WM_NAME,
                self.atoms.UTF8_STRING,
                title.as_bytes(),
            )
            .map_err(err)?
            .check()
            .map_err(err)?;

        Ok(self)
    }

    pub(crate) fn set_contents(&self, pixmap: &DrawTarget) -> Result<(), Error> {
        self.conn
            .put_image(
                ImageFormat::Z_PIXMAP,
                self.window,
                self.gc,
                pixmap.width().try_into().unwrap(),
                pixmap.height().try_into().unwrap(),
                0,
                0,
                0,
                24,
                pixmap.get_data_u8(),
            )
            .map_err(err)?
            .check()
            .map_err(err)?;
        Ok(())
    }

    /// Makes the window visible and raises it to the foreground.
    pub(crate) fn show(&self) -> Result<(), Error> {
        self.conn.map_window(self.window).map_err(err)?;
        self.conn
            .configure_window(
                self.window,
                &ConfigureWindowAux::new().stack_mode(StackMode::ABOVE),
            )
            .map_err(err)?;
        self.conn.flush().map_err(err)?;

        Ok(())
    }

    pub(crate) fn wait_for_event(&self) -> Result<WindowEvent, Error> {
        loop {
            let ev = self.conn.wait_for_event().map_err(err)?;
            if let Some(ev) = self.cvt_event(ev) {
                return Ok(ev);
            }
        }
    }

    pub(crate) fn poll_for_event(&self) -> Result<Option<WindowEvent>, Error> {
        loop {
            match self.conn.poll_for_event().map_err(err)? {
                Some(ev) => {
                    if let Some(ev) = self.cvt_event(ev) {
                        return Ok(Some(ev));
                    }
                }
                None => return Ok(None),
            }
        }
    }

    fn cvt_event(&self, ev: Event) -> Option<WindowEvent> {
        Some(match ev {
            Event::ClientMessage(msg) if msg.data.as_data32()[0] == self.atoms.WM_DELETE_WINDOW => {
                WindowEvent::CloseRequested
            }
            Event::KeyPress(press)
                if press.event == self.window
                    && press.detail == KEYCODE_ESC
                    && !press
                        .state
                        .intersects(KeyButMask::CONTROL | KeyButMask::SHIFT | KeyButMask::MOD1) =>
            {
                // ESC closes the dialog.
                WindowEvent::CloseRequested
            }
            Event::Expose(ex) if ex.count == 0 => WindowEvent::RedrawRequested,
            Event::EnterNotify(e) => WindowEvent::CursorEnter(CursorPos {
                x: e.event_x,
                y: e.event_y,
            }),
            Event::LeaveNotify(_) => WindowEvent::CursorLeave,
            Event::MotionNotify(e) => WindowEvent::CursorMove(CursorPos {
                x: e.event_x,
                y: e.event_y,
            }),
            Event::ButtonPress(e) => mouse_button(e.detail).map(WindowEvent::ButtonPress)?,
            Event::ButtonRelease(e) => mouse_button(e.detail).map(WindowEvent::ButtonRelease)?,
            _ => return None,
        })
    }

    /// Initiates window dragging.
    pub(crate) fn start_drag(&self) -> Result<(), Error> {
        let pointer = self
            .conn
            .query_pointer(self.window)
            .map_err(err)?
            .reply()
            .map_err(err)?;

        let window_pos = self
            .conn
            .translate_coordinates(self.window, pointer.root, 0, 0)
            .map_err(err)?
            .reply()
            .map_err(err)?;

        let x = (window_pos.dst_x + pointer.win_x) as u32;
        let y = (window_pos.dst_y + pointer.win_y) as u32;

        self.conn
            .send_event(
                false,
                pointer.root,
                EventMask::SUBSTRUCTURE_NOTIFY | EventMask::SUBSTRUCTURE_REDIRECT,
                ClientMessageEvent::new(
                    32,
                    self.window,
                    self.atoms._NET_WM_MOVERESIZE,
                    [x, y, MOVERESIZE_MOVE, 1, 1],
                ),
            )
            .map_err(err)?
            .check()
            .map_err(err)?;

        Ok(())
    }
}

fn mouse_button(detail: u8) -> Option<MouseButton> {
    Some(match detail {
        1 => MouseButton::Left,
        2 => MouseButton::Middle,
        3 => MouseButton::Right,
        _ => return None,
    })
}
