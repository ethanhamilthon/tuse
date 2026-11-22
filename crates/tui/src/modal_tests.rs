use crate::app::App;
use crate::picker::{Kind, Picker, item};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn press(app: &mut App, code: KeyCode, mods: KeyModifiers) {
    app.key(KeyEvent::new(code, mods), 10);
}

fn labels(app: &App) -> Vec<String> {
    let p = app.picker.as_ref().unwrap();
    p.shown.iter().map(|&i| p.items[i].label.clone()).collect()
}

fn type_text(app: &mut App, s: &str) {
    for c in s.chars() {
        press(app, KeyCode::Char(c), KeyModifiers::NONE);
    }
}

#[test]
fn search_is_edited_in_the_editor_and_esc_restores_the_draft() {
    let mut app = crate::app::test_app("modal-search");
    app.paste("my draft");
    let items = ["alpha beta", "gallery", "/trace +lua"]
        .map(|s| item(s, "", s))
        .to_vec();
    app.open_modal(Picker::new(Kind::Resume, "Resume", items));
    assert!(app.editor.is_empty());

    type_text(&mut app, "al be");
    assert_eq!(app.editor.text(), "al be");
    assert_eq!(labels(&app), ["alpha beta"]);
    press(&mut app, KeyCode::Char('w'), KeyModifiers::CONTROL);
    assert_eq!(app.editor.text(), "al ");
    press(&mut app, KeyCode::Backspace, KeyModifiers::NONE);
    press(&mut app, KeyCode::Home, KeyModifiers::NONE);
    press(&mut app, KeyCode::Char('g'), KeyModifiers::NONE);
    assert_eq!(app.editor.text(), "gal");
    assert_eq!(labels(&app), ["gallery"]);

    // One line only; `/` does not open the command picker; Ctrl+C neither quits nor cancels.
    press(&mut app, KeyCode::Enter, KeyModifiers::SHIFT);
    press(&mut app, KeyCode::End, KeyModifiers::NONE);
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    app.paste("/trace\n+lu");
    assert_eq!(app.editor.text(), "/trace +lu");
    assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Resume);
    assert_eq!(labels(&app), ["/trace +lua"]);
    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    press(&mut app, KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(!app.quit && app.picker.is_some());

    press(&mut app, KeyCode::Esc, KeyModifiers::NONE);
    assert!(app.picker.is_none() && app.saved_editor.is_none());
    assert_eq!(app.editor.text(), "my draft");
    crate::app::cleanup(app);
}

#[test]
fn model_to_effort_keeps_the_draft_and_a_refresh_keeps_the_search() {
    let mut app = crate::app::test_app("modal-models");
    app.paste("draft");
    app.open_models();
    let (current, _) = app.session.model();
    type_text(&mut app, "zzz-none");
    assert!(app.picker.as_ref().unwrap().shown.is_empty());
    app.handle_event(tuse_core::Event::Models);
    assert_eq!(app.editor.text(), "zzz-none");
    assert!(app.picker.as_ref().unwrap().shown.is_empty());

    press(&mut app, KeyCode::Char('u'), KeyModifiers::CONTROL);
    app.paste(&current);
    app.handle_event(tuse_core::Event::Models);
    assert_eq!(app.editor.text(), current);
    assert_eq!(
        app.picker.as_ref().unwrap().current().unwrap().value,
        current
    );

    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(matches!(app.picker.as_ref().unwrap().kind, Kind::Effort(_)));
    assert!(app.editor.is_empty());
    press(&mut app, KeyCode::Enter, KeyModifiers::NONE);
    assert!(app.picker.is_none());
    assert_eq!(app.editor.text(), "draft");
    crate::app::cleanup(app);
}
