//! Handles drawing and layouting of the UI, and processes input events for the UI.

mod font;

use std::{cmp, f32::consts::PI};

use euclid::{Size2D, Transform2D, point2, size2};
use raqote::{
    BlendMode, Color, DrawTarget, IntPoint, IntRect, Path, PathBuilder, SolidSource, Source,
    StrokeStyle,
};

use crate::{Answer, Icon, MouseButton, Theme, WindowEvent, ui::font::Font};

#[derive(Debug, Clone, Copy)]
struct Rgb(u8, u8, u8);

impl From<Rgb> for Color {
    fn from(Rgb(r, g, b): Rgb) -> Self {
        Self::new(255, r, g, b)
    }
}
impl From<Rgb> for SolidSource {
    fn from(Rgb(r, g, b): Rgb) -> Self {
        Self::from_unpremultiplied_argb(255, r, g, b)
    }
}
impl From<Rgb> for Source<'_> {
    fn from(value: Rgb) -> Self {
        SolidSource::from(value).into()
    }
}

const fn rgb(r: u8, g: u8, b: u8) -> Rgb {
    Rgb(r, g, b)
}

struct Colors {
    window_bg: Rgb,
    text: Rgb,

    button: Rgb,
    button_hover: Rgb,
    button_pressed: Rgb,
    button_outline: Rgb,
}

static THEME_LIGHT: Colors = Colors {
    window_bg: rgb(230, 230, 230),
    text: rgb(0, 0, 0),
    button: rgb(210, 210, 210),
    button_hover: rgb(180, 180, 180),
    button_pressed: rgb(150, 150, 150),
    button_outline: rgb(40, 40, 40),
};
static THEME_DARK: Colors = Colors {
    window_bg: rgb(30, 30, 30),
    text: rgb(255, 255, 255),
    button: rgb(90, 90, 90),
    button_hover: rgb(110, 110, 110),
    button_pressed: rgb(160, 160, 160),
    button_outline: rgb(200, 200, 200),
};

const WINDOW_PADDING: i32 = 10;
const BTN_PADDING: i32 = 12;
const SPACING: i32 = 10;
const BTN_RADIUS: f32 = 5.0;

pub(crate) struct Ui {
    colors: &'static Colors,
    pub(crate) canvas: DrawTarget,
    icon: DrawTarget,
    icon_pos: IntPoint,
    message: DrawTarget,
    message_pos: IntPoint,
    buttons: Vec<Button>,
    cursor_pos: Option<IntPoint>,
    mouse_pressed: bool,
    mouse_dragging: bool,
}

struct Button {
    /// Text size + padding.
    min_size: Size2D<i32, ()>,
    size: Size2D<i32, ()>,
    pos: IntPoint,
    text: DrawTarget,
}

impl Button {
    fn contains(&self, pt: IntPoint) -> bool {
        pt.x >= self.pos.x
            && pt.y >= self.pos.y
            && pt.x < self.pos.x + self.size.width
            && pt.y < self.pos.y + self.size.height
    }
}

impl Ui {
    pub(crate) fn new(icon: Icon, theme: Theme, text: &str, buttons: &[&str]) -> Self {
        const MIN_WIDTH: i32 = 400;
        const MIN_HEIGHT: i32 = 100;

        let colors = match theme {
            Theme::Light => &THEME_LIGHT,
            Theme::Dark => &THEME_DARK,
        };

        let icon = icon.get();
        let font = Font::load();

        // Compute sizes of the individual components first.
        let message_pos_x = icon.width() + WINDOW_PADDING + SPACING;
        let message_space = MIN_WIDTH - message_pos_x - WINDOW_PADDING;
        let message = font
            .render(text)
            .with_max_width(message_space as f32)
            .with_color(colors.text)
            .finish();

        let mut btn_height = 0;
        let mut buttons = buttons
            .iter()
            .map(|txt| {
                let text = font.render(txt).with_color(colors.text).finish();
                let w = text.width() + 2 * BTN_PADDING;
                let h = text.height() + 2 * BTN_PADDING;
                btn_height = cmp::max(btn_height, h);
                Button {
                    min_size: size2(w, h),
                    size: Size2D::zero(),
                    pos: IntPoint::zero(),
                    text,
                }
            })
            .collect::<Vec<_>>();

        // Now we can compute the required window size.

        let mut win_width = cmp::max(
            MIN_WIDTH,
            icon.width() + SPACING + message.width() + 2 * WINDOW_PADDING,
        );
        let win_height_message = message.height() + btn_height + SPACING + 2 * WINDOW_PADDING;
        let win_height_icon = icon.height() + btn_height + SPACING * 2 + WINDOW_PADDING;
        let win_height = cmp::max(MIN_HEIGHT, cmp::max(win_height_message, win_height_icon));

        let message_space_y = win_height - btn_height - WINDOW_PADDING * 2 - SPACING;
        let message_pos_y = (message_space_y - message.height()) / 2 + WINDOW_PADDING;

        // Absolute minimum required width of the button row.
        let width_sum = buttons.iter().map(|btn| btn.min_size.width).sum::<i32>();
        let required_width =
            width_sum + 2 * WINDOW_PADDING + (buttons.len().saturating_sub(1) as i32) * SPACING;
        win_width = cmp::max(win_width, required_width);

        let mut x = WINDOW_PADDING;
        let btn_width = (win_width - WINDOW_PADDING * 2 - SPACING * (buttons.len() as i32 - 1))
            / buttons.len() as i32;
        for btn in &mut buttons {
            btn.size = size2(btn_width, btn_height);
            btn.pos = point2(x, win_height - WINDOW_PADDING - btn_height);

            x += btn.size.width + SPACING;
        }

        let mut this = Self {
            colors,
            canvas: DrawTarget::new(win_width, win_height),
            icon,
            icon_pos: point2(WINDOW_PADDING, WINDOW_PADDING),
            message,
            message_pos: point2(message_pos_x, message_pos_y),
            buttons,
            cursor_pos: None,
            mouse_pressed: false,
            mouse_dragging: false,
        };
        this.redraw();
        this
    }

    pub(crate) fn process_event(&mut self, event: WindowEvent) -> Option<Answer> {
        match event {
            WindowEvent::CloseRequested => return Some(Answer::Closed),
            WindowEvent::CursorEnter(pos) | WindowEvent::CursorMove(pos) => {
                self.cursor_pos = Some(point2(pos.x.into(), pos.y.into()));
                self.mouse_dragging = self.mouse_pressed;
            }
            WindowEvent::CursorLeave => self.cursor_pos = None,
            WindowEvent::ButtonPress(MouseButton::Left) => self.mouse_pressed = true,
            WindowEvent::ButtonRelease(MouseButton::Left) => {
                if let Some(p) = self.cursor_pos
                    && let Some(i) = self.buttons.iter().position(|btn| btn.contains(p))
                    && !self.mouse_dragging
                {
                    return Some(Answer::Button(i));
                }
                self.mouse_pressed = false;
                self.mouse_dragging = false;
            }
            _ => {}
        }

        None
    }

    pub(crate) fn redraw(&mut self) {
        self.canvas.clear(self.colors.window_bg.into());

        self.canvas.place_surface(&self.icon, self.icon_pos);
        self.canvas.place_surface(&self.message, self.message_pos);

        for btn in &self.buttons {
            let mut color = self.colors.button;
            if let Some(pos) = self.cursor_pos
                && btn.contains(pos)
            {
                color = if self.mouse_pressed {
                    self.colors.button_pressed
                } else {
                    self.colors.button_hover
                };
            }

            let mut pb = PathBuilder::new();
            pb.rect(
                btn.pos.x as f32,
                btn.pos.y as f32,
                btn.size.width as f32,
                btn.size.height as f32,
            );
            let path = rounded_rect(btn.size, BTN_RADIUS).transform(&Transform2D::translation(
                btn.pos.x as f32,
                btn.pos.y as f32,
            ));
            self.canvas.fill(&path, &color.into(), &Default::default());
            self.canvas.stroke(
                &path,
                &self.colors.button_outline.into(),
                &StrokeStyle::default(),
                &Default::default(),
            );

            let text_x = btn.pos.x + btn.size.width / 2 - btn.text.width() / 2;
            self.canvas
                .place_surface(&btn.text, point2(text_x, btn.pos.y + BTN_PADDING));
        }
    }
}

trait DrawTargetExt {
    /// Renders `surface` at `position` in `self`.
    ///
    /// The entire surface will be rendered, and this will use the standard `SrcOver` blend mode.
    fn place_surface(&mut self, surface: &DrawTarget, position: IntPoint);
}
impl DrawTargetExt for DrawTarget {
    fn place_surface(&mut self, surface: &DrawTarget, position: IntPoint) {
        self.blend_surface(
            surface,
            IntRect::from_size(size2(surface.width(), surface.height())),
            position,
            BlendMode::SrcOver,
        );
    }
}

fn rounded_rect(size: Size2D<i32, ()>, radius: f32) -> Path {
    let width = size.width as f32;
    let height = size.height as f32;

    let mut pb = PathBuilder::new();

    pb.move_to(radius, 0.0);
    pb.line_to(width - radius, 0.0);
    pb.arc(width - radius, radius, radius, -PI * 0.5, PI * 0.5);
    pb.line_to(width, height - radius);
    pb.arc(width - radius, height - radius, radius, 0.0, PI * 0.5);
    pb.line_to(radius, height);
    pb.arc(radius, height - radius, radius, PI * 0.5, PI * 0.5);
    pb.line_to(0.0, radius);
    pb.arc(radius, radius, radius, PI, PI * 0.5);

    pb.finish()
}
