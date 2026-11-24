use crate::app::App;
use crate::render::duration;
use crate::style::{LABEL_SEP, Palette, SPINNER, STEERING_FRAMES, STEERING_LABEL, Style};
use crate::text::{Line, span};

pub fn working(app: &App) -> Line {
    let p = &app.palette;
    let Some((elapsed, _)) = app.session.turn_info() else {
        return vec![];
    };
    if app.steering.is_some() {
        let wave = STEERING_FRAMES[app.tick % STEERING_FRAMES.len()];
        return vec![
            span(format!("{wave} "), Style::fg(p.accent)),
            span(STEERING_LABEL, Style::fg(p.muted)),
        ];
    }
    let spin = SPINNER[app.tick % SPINNER.len()];
    vec![span(
        format!("{spin} {}", duration(elapsed)),
        Style::fg(p.accent),
    )]
}

pub fn model(app: &App) -> Line {
    let (model, effort) = app.session.model();
    let def = app.session.model_def(&model);
    let stats = app.session.stats();
    let window = def.as_ref().map_or(0, |d| d.window);
    let context = (window > 0 || stats.context > 0).then_some((stats.context, window));
    let cost = stats.priced.then_some(stats.cost);
    model_line(
        &app.palette,
        &tuse_config::display_name(def.as_ref(), &model),
        &effort,
        context,
        cost,
    )
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_indicator_omits_transient_actions_and_keeps_steering_and_flash() {
        use std::net::TcpListener;
        use std::time::Duration;
        use tuse_core::{Event, Options, Session, UserInput};

        crate::app::test_home();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let dir = std::env::temp_dir().join(format!("tuse-work-ind-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let cfg = dir.join("tuse.lua");
        let addr = listener.local_addr().unwrap();
        tuse_core::add_provider(&tuse_config::Connect {
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
        let mut app = App::new(session.clone(), Some(cfg));
        assert!(working(&app).is_empty());

        let (id, _) = session.submit(UserInput::text("go"));
        loop {
            if matches!(app.events.recv_timeout(Duration::from_secs(5)).unwrap(), Event::User { id: got, .. } if got == id)
            {
                break;
            }
        }
        app.handle_event(Event::Status("reading a.txt...".into()));
        app.handle_event(Event::Status("running cargo test...".into()));

        let line = working(&app);
        assert_eq!(line.len(), 1);
        assert_eq!(line[0].style, Style::fg(app.palette.accent));
        assert!(line[0].text.starts_with(SPINNER[0]));
        assert!(
            !line[0].text.contains("·")
                && !line[0].text.contains("reading")
                && !line[0].text.contains("running")
        );

        app.tick = 1;
        assert!(working(&app)[0].text.starts_with(SPINNER[1]));

        app.steering = Some(id);
        let steered = working(&app);
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
        assert_eq!(working(&app).len(), 1);

        app.flash("Copied.");
        let top = crate::input_view::block(&app, 40, &[String::new()], 0);
        let top_text: String = top[0].iter().map(|s| s.text.as_str()).collect();
        assert!(top_text.contains("Copied.") && !top_text.contains(SPINNER[1]));
        app.flash = None;
        let top = crate::input_view::block(&app, 40, &[String::new()], 0);
        let top_text: String = top[0].iter().map(|s| s.text.as_str()).collect();
        assert!(top_text.contains(SPINNER[1]));

        session.close();
        drop(listener);
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
