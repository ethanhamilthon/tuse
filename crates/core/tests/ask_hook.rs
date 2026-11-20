mod common;
use common::Reply;
use serde_json::json;
use std::time::Duration;
use tuse_core::{Event, Mode, Options, Session, Start, UserInput};

#[test]
fn only_registered_main_questions_enqueue_a_hook() {
    for mode in [Mode::Json, Mode::Headless] {
        let code = if mode == Mode::Json {
            "task_run(\"ask('background')\")\nask('main', ['one', 'two'])"
        } else {
            "ask('main')"
        };
        let addr = common::provider(vec![
            Reply::Call("python", json!({"code":code,"title":"Ask"}).to_string()),
            Reply::Text("done"),
        ]);
        let (dir, cfg) = common::setup(
            "ask-hook",
            &addr,
            r#"hooks:
  observer:
    events: [ask, process_end]
    run: |
      if event == 'ask' and question == 'main':
          ask(f'{ask_id}: {question} {options}')
"#,
        );
        let s = Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg),
            mode,
            resume: None,
            start: Start::Launch,
        })
        .unwrap();
        let events = s.subscribe();
        s.submit(UserInput::text("begin"));
        let mut main = None;
        let mut observed = None;
        let mut background = false;
        loop {
            match events.recv_timeout(Duration::from_secs(10)).unwrap() {
                Event::Ask { id, question, .. } if question == "main" => main = Some(id),
                Event::Ask { id, question, .. } => {
                    if question == "background" {
                        background = true;
                    } else {
                        observed = Some(question);
                    }
                    s.answer(&id, Some("ok".into()));
                }
                Event::TurnEnd { .. } => break,
                _ => {}
            }
            if background
                && let (Some(id), Some(payload)) = (&main, &observed)
                && s.has_ask(id)
            {
                assert_eq!(payload, &format!("{id}: main ['one', 'two']"));
                s.answer(id, Some("private answer".into()));
            }
        }
        s.close();
        let runs: Vec<_> = s
            .trace()
            .into_iter()
            .filter(|x| x.summary.starts_with("observer ask "))
            .collect();
        assert_eq!(runs.len(), usize::from(mode == Mode::Json));
        if mode == Mode::Json {
            assert!(background && observed.is_some());
            assert!(!runs[0].text.contains("private answer"));
        }
        common::cleanup(&dir);
    }
}
