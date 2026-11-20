use super::*;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[test]
fn wrapped_question_keeps_its_viewport_in_the_answer_editor() {
    let mut app = crate::app::test_app("wrapped-ask");
    app.palette.ui.pad_x = 0;
    app.handle_event(tuse_core::Event::Ask {
        id: "a1".into(),
        question: "first\nabcdefghijklmnopqrst\nlast".into(),
        options: vec![],
    });
    let mut renderer = Renderer::default();
    let mut frame = |app: &mut App, h| {
        renderer.draw(app, &mut Vec::new(), 10, h).unwrap();
        renderer
            .prev
            .iter()
            .map(|line| line.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
    };
    let full = frame(&mut app, 16);
    assert!(
        full.windows(4)
            .any(|rows| rows == ["first", "abcdefghij", "klmnopqrst", "last"])
    );
    let before = frame(&mut app, 6);
    assert!(!before.iter().any(|s| s == "last"));
    let page = |app: &mut App, key| app.key(KeyEvent::new(key, KeyModifiers::NONE), 1);
    for _ in 0..5 {
        page(&mut app, KeyCode::PageDown);
    }
    let picker = frame(&mut app, 6);
    let row = picker.iter().position(|s| s == "last").unwrap();
    let offset = app.ask_scroll;
    app.choose();
    let editor = frame(&mut app, 6);
    assert_eq!(editor[row], "last");
    assert_eq!(app.ask_scroll, offset);
    page(&mut app, KeyCode::PageUp);
    let scrolled = frame(&mut app, 6);
    assert!(scrolled.iter().any(|s| s == "klmnopqrst"));
    assert!(app.ask_input.is_some());
    crate::app::cleanup(app);
}
