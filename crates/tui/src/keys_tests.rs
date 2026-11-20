use super::*;
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
        headless: false,
        resume: None,
    })
    .unwrap();
    let mut app = App::new(session.clone(), dir.clone(), Some(cfg));
    let press = |app: &mut App, code| app.key(KeyEvent::new(code, KeyModifiers::NONE), 0);
    app.editor.insert("draft /theme after");
    app.editor.cursor = 12;
    app.update_token_picker();
    assert_eq!(app.picker.as_ref().unwrap().kind, Kind::Command);
    press(&mut app, KeyCode::Enter);
    assert_eq!(app.editor.text(), "draft  after");
    assert_eq!(app.picker.take().unwrap().kind, Kind::Theme);
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
