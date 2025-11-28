use std::process;

use alerta::{Answer, ButtonPreset, Error, Icon, Theme};
use larpa::{
    Command,
    types::{Color, PrintVersion},
};

/// Alerta is a small command-line utility that can display simple message dialogs on the
/// local X11 or XWayland server.
///
/// Alerta will use the following exit status codes to communicate the result of the dialog:
/// - 0-N: Indicates that the button with this index was clicked to close the dialog (0 being the
///   leftmost button, etc.).
/// - 64: Indicates an error in the provided command-line arguments.
/// - 50: The dialog window was closed by other means than the displayed buttons (for example,
///   Alt+F4 or ESC).
/// - 100: An error occurred while displaying the dialog.
/// - 101: A panic occurred (this is a bug in Alerta, please file an issue).
#[derive(Command)]
struct Args {
    /// The message to display in the dialog.
    message: String,

    /// The window title.
    #[larpa(name = "--title")]
    title: Option<String>,

    /// The icon to display next to the message. [choices: info, warning, error, question]
    #[larpa(name = "--icon", default = "info")]
    icon: Icon,

    /// The set of buttons to display. [choices: close, ok, okcancel, retrycancel, yesno, yesnocancel]
    #[larpa(name = "--buttons", default = "close")]
    buttons: ButtonPreset,

    /// The theme to use. [choices: light, dark]
    #[larpa(name = "--theme")]
    theme: Option<Theme>,

    /// Whether to use ANSI colors in console output. [choices: always, auto, never]
    #[larpa(name = "--color", default)]
    _color: Color,

    /// Print version information.
    #[larpa(name = "--version", flag)]
    _version: PrintVersion,
}

fn main() {
    match run() {
        Ok(code) => process::exit(code),
        Err(e) => {
            eprintln!("error: {e}");
            process::exit(100);
        }
    }
}

fn run() -> Result<i32, Error> {
    let args = Args::from_args();

    let mut b = alerta::alerta()
        .message(args.message)
        .icon(args.icon)
        .button_preset(args.buttons);
    if let Some(title) = args.title {
        b = b.title(title);
    }
    if let Some(theme) = args.theme {
        b = b.theme(theme);
    }

    let ans = b.show()?;
    let exit_status = match ans {
        Answer::Closed => 50,
        Answer::Button(i) => i as i32,
    };

    Ok(exit_status)
}
