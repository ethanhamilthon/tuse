use crate::kit::rich::render::duration;
use crate::kit::style::{LABEL_SEP, Palette, SPINNER, STEERING_FRAMES, STEERING_LABEL, Style};
use crate::kit::text::{Line, span};
use crate::ui::app::App;

pub fn model_line(
    p: &Palette,
    name: &str,
    effort: &str,
    context: Option<(u64, u64)>,
    cost: Option<f64>,
) -> Line {
    let muted = Style::fg(p.muted);
    let mut text = format!("{name}{LABEL_SEP}{effort}");
    let mut line = vec![];
    if let Some((used, window)) = context {
        text.push_str(LABEL_SEP);
        line.push(span(std::mem::take(&mut text), muted));
        line.push(span(tuse_core::tokens(used), Style::fg(p.accent)));
        let max = if window > 0 {
            tuse_core::tokens(window)
        } else {
            "?".into()
        };
        text = format!("/{max}");
    }
    if let Some(cost) = cost {
        text.push_str(&format!("{LABEL_SEP}${cost:.2}"));
    }
    line.push(span(text, muted));
    line
}

impl App {
    pub(crate) fn working(&self) -> Line {
        let p = &self.palette;
        let Some((elapsed, _)) = self.session.turn_info() else {
            return vec![];
        };
        if self.steering.is_some() {
            let wave = STEERING_FRAMES[self.tick % STEERING_FRAMES.len()];
            return vec![
                span(format!("{wave} "), Style::fg(p.accent)),
                span(STEERING_LABEL, Style::fg(p.muted)),
            ];
        }
        use crate::kit::style::{STREAM_PULSE, TOOL_STARS};
        let frames = match self.activity {
            tuse_core::Activity::Provider if self.visible_stream => STREAM_PULSE,
            tuse_core::Activity::Provider => SPINNER,
            tuse_core::Activity::Lua => TOOL_STARS,
            _ => return vec![],
        };
        let spin = frames[self.tick % frames.len()];
        vec![span(
            format!("{spin} {}", duration(elapsed)),
            Style::fg(p.accent),
        )]
    }

    pub(crate) fn model_label(&self) -> Line {
        let (model, effort) = self.session.model();
        let def = self.session.model_def(&model);
        let stats = self.session.stats();
        let window = def.as_ref().map_or(0, |d| d.window);
        let context = (window > 0 || stats.context > 0).then_some((stats.context, window));
        let cost = stats.priced.then_some(stats.cost);
        model_line(
            &self.palette,
            &tuse_config::display_name(def.as_ref(), &model),
            &effort,
            context,
            cost,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_indicator_omits_transient_actions_and_keeps_steering_and_flash() {
        use std::io::{BufRead, BufReader, Read, Write};
        use std::net::TcpListener;
        use std::sync::mpsc::{channel, sync_channel};
        use std::time::{Duration, Instant};
        use tuse_core::{Event, Options, Session, UserInput};

        crate::ui::app::test_home();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = std::env::temp_dir().join(format!("tuse-work-ind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("tuse.lua");
        let addr = listener.local_addr().unwrap();
        tuse_core::Providers::add(&tuse_config::Connect {
            name: "workind".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
            api: "openai-chat".into(),
        })
        .unwrap();
        std::fs::write(&cfg, "return {}").unwrap();
        let session = Session::open(Options {
            cwd: dir.clone(),
            config: Some(cfg.clone()),
            mode: tuse_core::Mode::Tui,
            resume: None,
            start: Default::default(),
        })
        .unwrap();
        session.set_model("workind/m", "default");
        let (wake, wake_rx) = sync_channel(1);
        let mut app = App::with_wake(session.clone(), Some(cfg), Some(wake));
        assert!(app.working().is_empty());

        let (send_delta, await_delta) = channel();
        let (finish, await_finish) = channel();
        let server = std::thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let mut socket = BufReader::new(socket);
            let mut length = 0;
            loop {
                let mut line = String::new();
                assert!(socket.read_line(&mut line).unwrap() > 0);
                if line.trim().is_empty() {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            socket.read_exact(&mut vec![0; length]).unwrap();
            await_delta.recv_timeout(Duration::from_secs(5)).unwrap();
            let out = socket.get_mut();
            write!(
                out,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
            )
            .unwrap();
            out.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"first\"}}]}\n\n")
                .unwrap();
            out.flush().unwrap();
            await_finish.recv_timeout(Duration::from_secs(5)).unwrap();
            out.write_all(b"data: [DONE]\n\n").unwrap();
        });

        let (id, _) = session.submit(UserInput::text("go"));
        let mut tick = Instant::now();
        while app.activity != tuse_core::Activity::Provider {
            wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            app.pump(&mut tick);
        }
        app.handle_event(Event::Status("reading a.txt...".into()));
        app.handle_event(Event::Status("running cargo test...".into()));

        let line = app.working();
        assert_eq!(line.len(), 1);
        assert_eq!(line[0].style, Style::fg(app.palette.accent));
        assert!(line[0].text.starts_with(SPINNER[0]));
        assert!(
            !line[0].text.contains("·")
                && !line[0].text.contains("reading")
                && !line[0].text.contains("running")
        );

        app.tick = 1;
        assert!(app.working()[0].text.starts_with(SPINNER[1]));

        app.steering = Some(id);
        let steered = app.working();
        assert_eq!(
            steered,
            [
                span(
                    format!("{} ", STEERING_FRAMES[1]),
                    Style::fg(app.palette.accent)
                ),
                span(STEERING_LABEL, Style::fg(app.palette.muted)),
            ]
        );
        app.steering = None;
        assert_eq!(app.working().len(), 1);

        app.flash("Copied.");
        let top = app.input_block(&app.editor.view(40), 40, &[String::new()], 0);
        let top_text: String = top[0].iter().map(|s| s.text.as_str()).collect();
        assert!(top_text.contains("Copied.") && !top_text.contains(SPINNER[1]));
        app.flash = None;
        let top = app.input_block(&app.editor.view(40), 40, &[String::new()], 0);
        let top_text: String = top[0].iter().map(|s| s.text.as_str()).collect();
        assert!(top_text.contains(SPINNER[1]));
        send_delta.send(()).unwrap();
        while !app.visible_stream {
            wake_rx.recv_timeout(Duration::from_secs(5)).unwrap();
            tick = Instant::now();
            let pump = app.pump(&mut tick);
            if app.visible_stream {
                assert!(pump.dirty && pump.immediate);
            }
        }
        let frame = app.compose(50, 12);
        assert!(
            frame
                .screen
                .iter()
                .flatten()
                .any(|span| span.text == "first")
        );
        assert!(session.busy());
        assert!(
            app.working()[0]
                .text
                .starts_with(crate::kit::style::STREAM_PULSE[1])
        );
        app.activity = tuse_core::Activity::Lua;
        assert!(
            app.working()[0]
                .text
                .starts_with(crate::kit::style::TOOL_STARS[1])
        );
        app.activity = tuse_core::Activity::Idle;
        assert!(app.working().is_empty());

        finish.send(()).unwrap();
        server.join().unwrap();
        session.close();
        std::fs::remove_dir_all(tuse_core::tuse_home().join(&session.id)).ok();
        std::fs::remove_dir_all(dir).ok();
    }
    #[test]
    fn model_line_colors_only_the_used_context() {
        let p = Palette::from_config(&tuse_config::Config::default());
        let (m, a) = (Style::fg(p.muted), Style::fg(p.accent));
        assert_eq!(
            model_line(&p, "m", "high", Some((5200, 200_000)), Some(1.5)),
            [
                span("m · high · ", m),
                span("5.2k", a),
                span("/200k · $1.50", m)
            ]
        );
        assert_eq!(
            model_line(&p, "m", "low", Some((10, 0)), None),
            [span("m · low · ", m), span("10", a), span("/?", m)]
        );
        assert_eq!(model_line(&p, "m", "low", None, None), [span("m · low", m)]);
    }

    #[test]
    fn sizes_format() {
        assert_eq!(tuse_core::tokens(5200), "5.2k");
        assert_eq!(tuse_core::tokens(1_000_000), "1m");
        assert_eq!(tuse_core::tokens(200_000), "200k");
        assert_eq!(tuse_core::tokens(999), "999");
    }
}
