//! A library for creating GUI message dialogs.

mod error;
#[cfg(test)]
mod tests;
mod ui;
mod x11;

use std::{fmt, process::Command, str::FromStr};

pub use error::Error;
use rapid_qoi::Qoi;
use raqote::DrawTarget;

use crate::{error::err, ui::Ui, x11::X11Window};

/// Returns a [`Builder`] for creating dialogs.
///
/// This is the main entry point into the library.
pub fn alerta() -> Builder {
    Builder {
        title: None,
        message: None,
        theme: None,
        icon: Default::default(),
        button_preset: ButtonPreset::default(),
    }
}

/// A message dialog builder.
pub struct Builder {
    title: Option<String>,
    message: Option<String>,
    theme: Option<Theme>,
    icon: Icon,
    button_preset: ButtonPreset,
}

impl Builder {
    /// Sets the window title of the dialog.
    ///
    /// By default, the title is generated from the [`Icon`] the dialog uses.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Sets the message body.
    ///
    /// The body can contain line breaks, and will be line-wrapped to fit into the dialog window.
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Sets the icon to display in the dialog.
    ///
    /// By default, [`Icon::Info`] is used.
    pub fn icon(mut self, icon: Icon) -> Self {
        self.icon = icon;
        self
    }

    /// Sets the dialog's color theme.
    ///
    /// By default, the OS theme is used.
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = Some(theme);
        self
    }

    /// Sets the button preset to use.
    ///
    /// By default, [`ButtonPreset::Close`] is used.
    pub fn button_preset(mut self, preset: ButtonPreset) -> Self {
        self.button_preset = preset;
        self
    }

    /// Displays the dialog and blocks until the dialog is closed.
    ///
    /// Returns an [`Answer`] indicating which dialog button was clicked.
    ///
    /// # Errors
    ///
    /// An error may occur when communicating with the X server.
    pub fn show(self) -> Result<Answer, Error> {
        let title = match self.title {
            Some(title) => title,
            None => match self.icon {
                Icon::Error => "Error\0".into(),
                Icon::Warning => "Warning\0".into(),
                Icon::Info => "Info\0".into(),
                Icon::Question => "Question\0".into(),
            },
        };

        let mut ui = Ui::new(
            self.icon,
            self.theme.unwrap_or_else(Theme::detect),
            &self.message.unwrap_or_default(),
            self.button_preset.strings(),
        );

        let conn = x11::Connection::connect()?;

        let win = X11Window::create(
            conn.clone(),
            ui.canvas.width() as u16,
            ui.canvas.height() as u16,
        )?
        .with_title(title)?;

        win.set_contents(&ui.canvas)?;

        win.show()?;

        let mut pressed = false;
        loop {
            let mut process_event = |event| {
                match event {
                    WindowEvent::CursorMove(..) if pressed => {
                        win.start_drag().ok();
                    }
                    WindowEvent::ButtonPress(MouseButton::Left) => pressed = true,
                    WindowEvent::ButtonRelease(MouseButton::Left) => pressed = false,
                    _ => {}
                }
                ui.process_event(event)
            };

            let event = win.wait_for_event()?;
            if let Some(answer) = process_event(event) {
                return Ok(answer);
            }
            // Batch all pending events together to limit the number of redraws.
            while let Some(event) = win.poll_for_event()? {
                if let Some(answer) = process_event(event) {
                    return Ok(answer);
                }
            }

            ui.redraw();
            win.set_contents(&ui.canvas)?;
        }
    }
}

/// A user response to a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Answer {
    /// The dialog window was closed by the OS.
    ///
    /// This happens when the user clicks the close button in the window frame, presses Alt+F4
    /// (if the desktop environment is configured that way), or when some other mechanism causes the
    /// window manager or compositor to close the window.
    Closed,

    /// One of the dialog buttons was pressed.
    ///
    /// The 0-based button index is provided in the payload.
    Button(usize),
}

/// Presets of button groups.
///
/// These presets define a couple of well-established button combinations, in the order that users
/// expect.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ButtonPreset {
    #[default]
    Close,
    Ok,
    OkCancel,
    RetryCancel,
    YesNo,
    YesNoCancel,
}

impl ButtonPreset {
    fn strings(&self) -> &[&str] {
        match self {
            ButtonPreset::Close => &["Close"],
            ButtonPreset::Ok => &["OK"],
            ButtonPreset::OkCancel => &["OK", "Cancel"],
            ButtonPreset::RetryCancel => &["Retry", "Cancel"],
            ButtonPreset::YesNo => &["Yes", "No"],
            ButtonPreset::YesNoCancel => &["Yes", "No", "Cancel"],
        }
    }
}

impl FromStr for ButtonPreset {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "close" => Self::Close,
            "ok" => Self::Ok,
            "okcancel" => Self::OkCancel,
            "retrycancel" => Self::RetryCancel,
            "yesno" => Self::YesNo,
            "yesnocancel" => Self::YesNoCancel,
            _ => return Err(InvalidValue { _p: () }),
        })
    }
}

/// The icon to display in the dialog.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Icon {
    Error,
    Warning,
    #[default]
    Info,
    Question,
}

impl FromStr for Icon {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "error" => Self::Error,
            "warning" => Self::Warning,
            "info" => Self::Info,
            "question" => Self::Question,
            _ => return Err(InvalidValue { _p: () }),
        })
    }
}

impl Icon {
    fn get(self) -> DrawTarget {
        let src: &[u8] = match self {
            Icon::Error => include_bytes!("../../../3rdparty/icons/dialog-error.qoi"),
            Icon::Warning => include_bytes!("../../../3rdparty/icons/dialog-warning.qoi"),
            Icon::Info => include_bytes!("../../../3rdparty/icons/dialog-information.qoi"),
            Icon::Question => include_bytes!("../../../3rdparty/icons/dialog-question.qoi"),
        };

        let mut qoi = Qoi::decode_header(src).unwrap();
        qoi.colors = rapid_qoi::Colors::Rgba;

        let mut target = DrawTarget::new(qoi.width as _, qoi.height as _);
        Qoi::decode(src, target.get_data_u8_mut()).unwrap();

        // RGBA -> ARGB and premultiply.
        for p in target.get_data_mut() {
            let [mut r, mut g, mut b, a] = p.to_ne_bytes().map(u32::from);
            r = r * a / 255;
            g = g * a / 255;
            b = b * a / 255;
            *p = (a << 24) | (r << 16) | (g << 8) | b;
        }

        target
    }
}

/// Error returned by [`FromStr`] implementations.
#[derive(Debug)]
pub struct InvalidValue {
    _p: (),
}

impl fmt::Display for InvalidValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid value")
    }
}
impl std::error::Error for InvalidValue {}

/// Color theme selection.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Theme {
    #[default]
    Light,
    Dark,
}

impl FromStr for Theme {
    type Err = InvalidValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "light" => Self::Light,
            "dark" => Self::Dark,
            _ => return Err(InvalidValue { _p: () }),
        })
    }
}

impl Theme {
    /// Attempts to detect the preferred color scheme of the desktop environment.
    ///
    /// X11 doesn't have a built-in mechanism to do this, so we run these external commands in order
    /// (using the first one that works):
    ///
    /// - `dbus-send org.freedesktop.portal.Settings.Read org.freedesktop.appearance color-scheme`
    /// - `gsettings get org.gnome.desktop.interface color-scheme`
    ///
    /// The latter doesn't properly update on KDE when changing themes, so the first option is
    /// preferred (and is also generally newer and less vendor-specific).
    fn detect() -> Theme {
        Self::detect_dbus()
            .or_else(|_| Self::detect_gsettings())
            .unwrap_or(Theme::Light)
    }

    fn detect_dbus() -> Result<Theme, Error> {
        // https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Settings.html
        // Invocation taken from `sctk-adwaita`.
        let out = Command::new("dbus-send")
            .args([
                "--reply-timeout=100",
                "--print-reply=literal",
                "--dest=org.freedesktop.portal.Desktop",
                "/org/freedesktop/portal/desktop",
                "org.freedesktop.portal.Settings.Read",
                "string:org.freedesktop.appearance",
                "string:color-scheme",
            ])
            .output()
            .map_err(err)?;
        if !out.status.success() {
            return Err(Error::new(format!(
                "failed to query DBus: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }

        let stdout = str::from_utf8(&out.stdout).map_err(err)?.trim();
        if stdout.ends_with("uint32 1") {
            Ok(Theme::Dark)
        } else if stdout.ends_with("uint32 2") {
            Ok(Theme::Light)
        } else {
            Err(Error::new(format!(
                "unknown color scheme preference: {stdout}"
            )))
        }
    }

    fn detect_gsettings() -> Result<Theme, Error> {
        let out = Command::new("gsettings")
            .args(["get", "org.gnome.desktop.interface", "color-scheme"])
            .output()
            .map_err(err)?;
        if !out.status.success() {
            return Err(Error::new(format!(
                "failed to query gsettings: {}",
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        let stdout = str::from_utf8(&out.stdout)
            .map_err(err)?
            .trim()
            .trim_matches('\'');
        match stdout {
            "prefer-dark" => Ok(Theme::Dark),
            "prefer-light" => Ok(Theme::Light),
            _ => Err(Error::new(format!(
                "unknown color scheme preference: {stdout}"
            ))),
        }
    }
}

#[derive(Debug)]
enum WindowEvent {
    CloseRequested,
    RedrawRequested,
    CursorEnter(CursorPos),
    CursorMove(CursorPos),
    CursorLeave,
    ButtonPress(MouseButton),
    ButtonRelease(MouseButton),
}

#[derive(Debug, Clone, Copy)]
struct CursorPos {
    x: i16,
    y: i16,
}

#[derive(Debug)]
enum MouseButton {
    Left,
    Middle,
    Right,
}
