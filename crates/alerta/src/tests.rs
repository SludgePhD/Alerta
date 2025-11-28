use std::fs;

use raqote::DrawTarget;

use crate::{ButtonPreset, Icon, Theme, ui::Ui};

fn snap(name: &str, image: &DrawTarget) {
    let path = format!("src/snap/{name}.png");
    let old_contents = fs::read(&path).unwrap_or_default();
    image.write_png(&path).unwrap();

    let new_contents = fs::read(&path).expect("file should be readable now");

    assert!(
        old_contents == new_contents,
        "snapshot '{name}' was modified; rerun test to pass"
    );
}

const IPSUM: &str = "Lorem ipsum is a dummy or placeholder text commonly used in graphic design, publishing, and web development. Its purpose is to permit a page layout to be designed, independently of the copy that will subsequently populate it, or to demonstrate various fonts of a typeface without meaningful text that could be distracting.";
const NBSP: &str = "\u{a0}";
const ZWSP: &str = "\u{200b}";

#[test]
fn textwrap() {
    snap(
        "textwrap",
        &Ui::new(Icon::Info, Theme::Light, IPSUM, &["OK"]).canvas,
    );
    snap(
        "nbsp",
        &Ui::new(
            Icon::Info,
            Theme::Light,
            &IPSUM
                .split_inclusive('.')
                .next()
                .unwrap()
                .replace(' ', NBSP),
            &["OK"],
        )
        .canvas,
    );
    snap(
        "zwsp",
        &Ui::new(Icon::Info, Theme::Light, &IPSUM.replace(' ', ZWSP), &["OK"]).canvas,
    );
}

#[test]
fn buttons() {
    snap(
        "buttons-yesnocancel",
        &Ui::new(
            Icon::Warning,
            Theme::Light,
            "Buttons",
            ButtonPreset::YesNoCancel.strings(),
        )
        .canvas,
    );
    snap(
        "buttons-retrycancel",
        &Ui::new(
            Icon::Warning,
            Theme::Light,
            "Buttons",
            ButtonPreset::RetryCancel.strings(),
        )
        .canvas,
    );
}

#[test]
fn dark_theme() {
    snap(
        "dark-theme",
        &Ui::new(Icon::Question, Theme::Dark, IPSUM, &["Yes", "No", "Cancel"]).canvas,
    );
}

#[test]
fn icons() {
    snap(
        "icon-error",
        &Ui::new(
            Icon::Error,
            Theme::Light,
            "Error",
            ButtonPreset::RetryCancel.strings(),
        )
        .canvas,
    );
    snap(
        "icon-question",
        &Ui::new(
            Icon::Question,
            Theme::Light,
            "Huh? Wha?",
            ButtonPreset::YesNo.strings(),
        )
        .canvas,
    );
}
