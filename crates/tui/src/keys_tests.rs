use super::*;
use crate::expand_prompts::expand_prompts;
use crate::picker::Kind;
use std::collections::BTreeMap;
use std::net::TcpListener;
use std::time::Duration;
use tuse_core::{Event, Options, Session, UserInput};

#[test]
fn expands_prompt_tokens() {
    let mut p = BTreeMap::new();
    p.insert("non-stop".to_string(), "Do it all.".to_string());
    assert_eq!(expand_prompts("fix #non-stop", &p), "fix Do it all.");
    assert_eq!(expand_prompts("a#non-stop #x", &p), "a#non-stop #x");
}

#[test]
fn editing_withdraws_only_the_waiting_tui_message() {
    crate::app::test_home();
    // Hold the first request open so the worker cannot take the steering messages.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let dir = std::env::temp_dir().join(format!("tuse-steering-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let cfg = dir.join("tuse.yaml");
    let addr = listener.local_addr().unwrap();
    std::fs::write(
        &cfg,
        format!("model: p/m\nconnect: [{{name: p, base_url: 'http://{addr}', api_key: test}}]\n"),
    )
    .unwrap();
    let session = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: tuse_core::Mode::Tui,
        resume: None,
        start: Default::default(),
    })
    .unwrap();
    let mut app = App::new(session.clone(), dir.clone(), Some(cfg));
    let press = |app: &mut App, code| app.key(KeyEvent::new(code, KeyModifiers::NONE), 0);
    app.editor.insert("draft /theme after");
    app.editor.cursor = 12;
    app.update_token_picker();
    assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Command);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Theme);
    assert!(app.editor.is_empty());
    press(&mut app, KeyCode::Esc);
    assert!(app.picker.is_none());
    assert_eq!(app.editor.text(), "draft  after");
    app.editor.clear();
    let (first, _) = session.submit(UserInput::text("first"));
    loop {
        if matches!(app.events.recv_timeout(Duration::from_secs(5)).unwrap(),
            Event::User { id, .. } if id == first)
        {
            break;
        }
    }

    let (other, busy) = session.submit(UserInput::text("waiting"));
    assert!(busy);
    app.paste("waiting");
    press(&mut app, KeyCode::Enter);
    let waiting = app.steering.unwrap();
    press(&mut app, KeyCode::Left);
    assert_eq!(app.steering, Some(waiting));
    press(&mut app, KeyCode::End);
    press(&mut app, KeyCode::Char('!'));
    assert!(app.steering.is_none());
    assert_eq!(app.editor.text(), "waiting!");
    assert!(!session.withdraw_input(waiting));
    assert!(session.withdraw_input(other));
    assert!(session.busy());

    press(&mut app, KeyCode::Enter);
    let edited = app.steering.unwrap();
    assert_ne!(edited, waiting);
    assert!(session.withdraw_input(edited));

    // An edit after the queue no longer contains the ID must survive a late User event.
    app.paste("?");
    app.handle_event(Event::User {
        id: edited,
        text: "waiting!".into(),
        images: 0,
    });
    assert_eq!(app.editor.text(), "waiting!?");
    assert!(app.steering.is_none());

    session.close();
    drop(listener);
    std::fs::remove_dir_all(tuse_core::tuse_home().join(&session.id)).unwrap();
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn shortcuts_copy_cut_and_select_without_losing_input() {
    let mut app = crate::app::test_app("selection");
    let press = |app: &mut App, code, mods| app.key(KeyEvent::new(code, mods), 10);
    for primary in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
        app.paste("alpha beta");
        let word = KeyModifiers::SHIFT | KeyModifiers::ALT;
        press(&mut app, KeyCode::Left, word);
        assert_eq!(app.editor.selected_text().as_deref(), Some("beta"));
        press(&mut app, KeyCode::Char('c'), primary);
        crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "beta"));
        press(&mut app, KeyCode::Char('x'), primary);
        assert_eq!(app.editor.text(), "alpha beta");
        press(&mut app, KeyCode::Char('a'), primary);
        press(&mut app, KeyCode::Char('x'), primary);
        assert!(app.editor.is_empty());
        crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "alpha beta"));
        app.paste("clear");
        press(&mut app, KeyCode::Char('c'), primary);
        assert!(app.editor.is_empty() && !app.quit);
    }
    crate::app::cleanup(app);
}

#[test]
fn russian_layout_preserves_typing_and_routes_modified_keys() {
    let mut app = crate::app::test_app("layout");
    let press = |app: &mut App, c, mods| app.key(KeyEvent::new(KeyCode::Char(c), mods), 10);
    press(&mut app, 'ф', KeyModifiers::NONE);
    assert_eq!(app.editor.text(), "ф");
    press(&mut app, 'ф', KeyModifiers::CONTROL);
    assert_eq!(app.editor.selected_text().as_deref(), Some("ф"));
    press(&mut app, 'С', KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    crate::clipboard::TEST_TEXT.with(|s| assert_eq!(&*s.borrow(), "ф"));
    crate::app::cleanup(app);
}

#[test]
fn streamed_text_is_not_repeated_after_a_tool_delta() {
    let mut app = crate::app::test_app("dup");
    let n = app.chat.len();
    app.handle_event(Event::Text("Let me check.".into()));
    app.handle_event(Event::ToolDelta {
        index: 0,
        id: "c1".into(),
        name: "python".into(),
        code: "1".into(),
        title: None,
    });
    app.handle_event(Event::Assistant {
        text: "Let me check.".into(),
    });
    let texts = (n..app.chat.len())
        .filter(|&i| matches!(app.chat.get_mut(i), Some(crate::chat::Entry::Assistant(_))))
        .count();
    assert_eq!(texts, 1);
    // Without streaming the final text still shows up once.
    app.handle_event(Event::Assistant {
        text: "Done.".into(),
    });
    assert!(
        matches!(app.chat.get_mut(app.chat.len() - 1), Some(crate::chat::Entry::Assistant(t)) if t == "Done.")
    );
    crate::app::cleanup(app);
}
