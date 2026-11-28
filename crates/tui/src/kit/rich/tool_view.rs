use super::render::{chat_inset, chat_marker};
use crate::kit::style::{MORE, Palette, Style, TOOL_PREFIX};
use crate::kit::text::{Line, clip, span, width};
use tuse_core::FileDiff;

pub fn tool(
    input: &str,
    output: Option<&str>,
    code: Option<i32>,
    diffs: &[FileDiff],
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let failed = output.is_some() && code != Some(0);
    let head = Style::fg(if failed { p.error } else { p.fg });
    let lines = if input.trim().is_empty() {
        vec![]
    } else {
        source(input.trim(), true, width, head, failed, p)
    };
    finish(lines, output, diffs, width, p)
}

pub fn finish(
    mut lines: Vec<Line>,
    output: Option<&str>,
    diffs: &[FileDiff],
    width: usize,
    p: &Palette,
) -> Vec<Line> {
    let pad = " ".repeat(chat_inset(width));
    let keep = tuse_config::limit(p.ui.lines.tool, lines.len());
    if keep < lines.len() {
        lines.truncate(keep);
        if keep > 0 {
            lines.push(vec![span(format!("{pad}{MORE}"), Style::fg(p.border))]);
        }
    }
    let Some(out) = output else { return lines };
    let out = crate::kit::text::clean(out);
    let all: Vec<&str> = out.trim_end().lines().collect();
    let shown = tuse_config::limit(p.ui.lines.output, all.len());
    let st = Style::fg(p.muted);
    let room = width.saturating_sub(pad.len());
    for l in all.iter().take(shown) {
        lines.push(vec![span(pad.clone(), st), span(clip(l, room, MORE), st)]);
    }
    if shown > 0 && shown < all.len() {
        let more = format!("{pad}{MORE} {} more lines", all.len() - shown);
        lines.push(vec![span(more, Style::fg(p.border))]);
    }
    for d in diffs {
        lines.extend(diff(d, width, &pad, p));
    }
    lines
}

pub fn source(
    input: &str,
    first: bool,
    w: usize,
    head: Style,
    failed: bool,
    p: &Palette,
) -> Vec<Line> {
    let cleaned = crate::kit::text::clean(input);
    let code = match usize::try_from(p.ui.lines.strings) {
        Ok(keep) => crate::kit::rich::fold::fold_multiline_strings(&cleaned, keep),
        Err(_) => cleaned,
    };
    let pad = " ".repeat(chat_inset(w));
    let colored = if failed {
        code.split('\n').map(|l| vec![span(l, head)]).collect()
    } else {
        crate::kit::rich::syntax::highlight(&code, head, p)
    };
    let (lead, rest) = (
        vec![span(
            chat_marker(TOOL_PREFIX, w),
            Style::fg(if failed { p.error } else { p.icons }),
        )],
        vec![span(pad, head)],
    );
    colored
        .into_iter()
        .enumerate()
        .flat_map(|(i, l)| {
            let lead = if i == 0 && first { &lead } else { &rest };
            crate::kit::text::wrap(&l, w, lead, &rest)
        })
        .collect()
}

fn diff(d: &FileDiff, w: usize, pad: &str, p: &Palette) -> Vec<Line> {
    if p.ui.lines.diff == 0 {
        return vec![];
    }
    let room = w.saturating_sub(pad.len());
    let all: Vec<&str> = d.diff.lines().collect();
    let hdr = span(
        clip(&crate::kit::text::clean(&d.path), room, MORE),
        Style::fg(p.border),
    );
    let mut out = vec![vec![span(pad.to_string(), Style::fg(p.border)), hdr]];
    let shown = tuse_config::limit(p.ui.lines.diff, all.len());
    let pairs = pair_windows(&all[..shown], room, MORE);
    for (l, text) in all.iter().zip(pairs) {
        let color = match l.chars().next() {
            Some('+') => p.added,
            Some('-') => p.removed,
            _ => p.muted,
        };
        let st = Style::fg(color);
        out.push(vec![span(pad.to_string(), st), span(text, st)]);
    }
    if shown < all.len() {
        let more = format!("{pad}{MORE} {} more lines", all.len() - shown);
        out.push(vec![span(more, Style::fg(p.border))]);
    }
    out
}

fn pair_windows(lines: &[&str], room: usize, more: &str) -> Vec<String> {
    let mut out: Vec<String> = lines
        .iter()
        .map(|l| clip(&crate::kit::text::clean(l), room, more))
        .collect();
    for i in 0..lines.len().saturating_sub(1) {
        let (a, b) = (
            crate::kit::text::clean(lines[i]),
            crate::kit::text::clean(lines[i + 1]),
        );
        if !(a.starts_with('-') && b.starts_with('+')) || width(&a) <= room && width(&b) <= room {
            continue;
        }
        let gutter = a.find("| ").map_or(0, |g| a[..g + 2].chars().count());
        let same = a.chars().skip(gutter).zip(b.chars().skip(gutter));
        let at = gutter + same.take_while(|(x, y)| x == y).count();
        let start = if at + 8 > room {
            at.saturating_sub(room / 2).max(gutter)
        } else {
            0
        };
        for (j, s) in [(i, &a), (i + 1, &b)] {
            let head: String = s.chars().take(gutter).collect();
            let body: String = s.chars().skip(start.max(gutter)).collect();
            let lead = if start > gutter { more } else { "" };
            let text = format!("{head}{lead}{body}");
            out[j] = clip(&text, room, more);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_at_the_end_of_a_long_line_stays_visible() {
        let a = format!("- {:>4} | {}old", 1, "x".repeat(80));
        let b = format!("+ {:>4} | {}new", 1, "x".repeat(80));
        let got = pair_windows(&[&a, &b], 40, "…");
        assert!(
            got[0].ends_with("old") && got[1].ends_with("new"),
            "{got:?}"
        );
        assert!(got.iter().all(|l| width(l) <= 40));
    }

    #[test]
    fn output_is_muted_and_cut_marks_are_border() {
        let p = Palette::new(&Default::default());
        let fg = |l: &Line| l.last().unwrap().style.fg;
        let lines = tool(
            "a = 1\nb = 2",
            Some("one\ntwo\nthree"),
            Some(0),
            &[],
            40,
            &p,
        );
        let n = lines.len();
        assert_eq!(fg(&lines[n - 2]), p.muted, "the shown output line");
        assert!(lines[n - 1].last().unwrap().text.ends_with("2 more lines"));
        assert_eq!(fg(&lines[n - 1]), p.border, "`N more lines`");

        let ui = tuse_config::Ui {
            lines: tuse_config::Lines {
                tool: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let p = Palette::new(&ui);
        let lines = tool("a = 1\nb = 2", None, None, &[], 40, &p);
        assert_eq!(lines.len(), 2);
        assert!(lines[1][0].text.ends_with(MORE));
        assert_eq!(fg(&lines[1]), p.border, "the cut code mark");

        let d = FileDiff {
            path: "f".into(),
            diff: (0..20).map(|i| format!("+ {i}\n")).collect(),
        };
        let lines = tool("x", Some(""), Some(0), &[d], 40, &p);
        let last = lines.last().unwrap();
        assert!(last[0].text.ends_with("8 more lines"), "{last:?}");
        assert_eq!(fg(last), p.border, "`N more lines` under a diff");
    }

    #[test]
    fn tool_output_tabs_do_not_reach_the_terminal() {
        let p = Palette::new(&Default::default());
        let lines = tool("x = 1", Some("1\tx\n2\t\x1b[31my"), Some(0), &[], 40, &p);
        let text: String = lines.iter().flatten().map(|s| s.text.as_str()).collect();
        assert!(!text.contains('\t') && !text.contains('\x1b'), "{text:?}");
        assert!(text.contains("1    x"), "tabs become spaces: {text:?}");
    }

    #[test]
    fn diff_line_zero_hides_entire_diff_block_and_updates_on_reload_and_resume() {
        use crate::kit::entry::Entry;
        use crate::ui::app::App;
        use crate::ui::chat::Chat;
        use std::io::{BufRead, BufReader, Read, Write};
        use tuse_config::{Lines, Ui};
        use tuse_core::{Event, Options, Record, Session};

        let make_diff = |path: &str, n: usize| FileDiff {
            path: path.into(),
            diff: (0..n).map(|i| format!("+ {i}\n")).collect(),
        };
        let render_text = |lines: &[Line]| -> String {
            lines
                .iter()
                .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        };
        let pal = |ui| Palette::new(&ui);
        let diffs = vec![
            make_diff("a.rs", 10),
            make_diff("b.rs", 5),
            FileDiff {
                path: "empty.rs".into(),
                diff: String::new(),
            },
        ];
        let hidden = tool(
            "x = 1",
            Some("edited a.rs"),
            Some(0),
            &diffs,
            60,
            &pal(Ui {
                lines: Lines {
                    diff: 0,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
        let ht = render_text(&hidden);
        assert_eq!(hidden.len(), 2);
        assert!(
            ht.contains("x = 1")
                && ht.contains("edited a.rs")
                && !ht.contains("b.rs")
                && !ht.contains("empty.rs")
                && !ht.contains("more lines")
        );

        let p_zero = pal(Ui {
            lines: Lines {
                tool: 0,
                output: 0,
                diff: 0,
                ..Default::default()
            },
            ..Default::default()
        });
        let mut c = Chat::default();
        c.push(Entry::Notice("before".into()));
        c.push(Entry::Tool {
            id: "t1".into(),
            input: "x = 1".into(),
            output: Some("ok".into()),
            code: Some(0),
            diffs,
        });
        c.push(Entry::Notice("after".into()));
        assert_eq!(c.window(60, &p_zero, 0, 10).0.len(), 3);

        let two = tool(
            "x",
            Some(""),
            Some(0),
            &[make_diff("a.rs", 10)],
            60,
            &pal(Ui {
                lines: Lines {
                    diff: 2,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
        assert_eq!(two.len(), 5);
        assert!(render_text(&two).contains("8 more lines"));
        let all = tool(
            "x",
            Some(""),
            Some(0),
            &[make_diff("a.rs", 20)],
            60,
            &pal(Ui {
                lines: Lines {
                    diff: -1,
                    ..Default::default()
                },
                ..Default::default()
            }),
        );
        assert_eq!(all.len(), 22);
        assert!(render_text(&all).contains("+ 19") && !render_text(&all).contains("more lines"));

        let mut app = crate::ui::app::test_app("diff-reload");
        let cfg = app.cwd.join("tuse.lua");
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let mut step = 0;
            for mut s in listener.incoming().flatten() {
                let mut r = BufReader::new(s.try_clone().unwrap());
                let mut len = 0;
                let mut first = String::new();
                loop {
                    let mut line = String::new();
                    r.read_line(&mut line).unwrap();
                    if first.is_empty() {
                        first = line.clone();
                    }
                    if line.trim().is_empty() {
                        break;
                    }
                    if let Some(v) = line.to_lowercase().strip_prefix("content-length:") {
                        len = v.trim().parse().unwrap();
                    }
                }
                if first.starts_with("GET") {
                    let _ = write!(
                        s,
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{{\"data\":[]}}"
                    );
                    continue;
                }
                r.read_exact(&mut vec![0; len]).unwrap();
                step += 1;
                let body = if step == 1 {
                    r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"c1","function":{"name":"lua","arguments":"{\"code\":\"local t={} for i=0,19 do t[#t+1]='row'..i end fs.write('long.rs', table.concat(t, string.char(10)))\"}"}}]}}]}"#
                } else {
                    r#"{"choices":[{"delta":{"content":"done"}}]}"#
                };
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {body}\n\ndata: [DONE]\n\n"
                );
            }
        });
        tuse_core::Providers::add(&tuse_config::Connect {
            name: "difftest".into(),
            base_url: format!("http://{addr}"),
            api_key: "test".into(),
            api: "openai-chat".into(),
            ..Default::default()
        })
        .unwrap();
        let write_cfg = |n: i32| {
            std::fs::write(
                &cfg,
                format!(
                    "return {{ ui = {{ lines = {{ diff = {n} }} }}, test = {{ commands = {{ r = function() require('tuse').reload() end }} }} }}"
                ),
            )
            .unwrap();
        };
        let drain_until = |app: &mut App, pred: fn(&Event) -> bool| loop {
            let ev = app
                .events
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap();
            let done = pred(&ev);
            app.handle_event(ev);
            if done {
                break;
            }
        };
        write_cfg(12);
        app.run_command("reload");
        drain_until(&mut app, |e| matches!(e, Event::Reloaded));
        app.session.set_model("difftest/m", "medium");
        app.session.submit(tuse_core::UserInput::text("write"));
        drain_until(&mut app, |e| matches!(e, Event::TurnEnd { .. }));

        let view = |app: &mut App| render_text(&app.chat.window(60, &app.palette, 0, 60).0);
        assert!(view(&mut app).contains("8 more lines"));
        write_cfg(0);
        app.run_command("test:r");
        drain_until(&mut app, |e| matches!(e, Event::Reloaded));
        let v0 = view(&mut app);
        assert!(!v0.contains("row0") && !v0.contains("more lines"));
        write_cfg(12);
        app.run_command("test:r");
        drain_until(&mut app, |e| matches!(e, Event::Reloaded));
        let v12 = view(&mut app);
        assert!(v12.contains("row0") && v12.contains("8 more lines") && !v12.contains("row19"));
        write_cfg(-1);
        app.run_command("reload");
        let v_all = view(&mut app);
        assert!(v_all.contains("row0") && v_all.contains("row19") && !v_all.contains("more lines"));
        write_cfg(0);
        app.run_command("reload");
        assert!(!view(&mut app).contains("row0"));

        let (sid, cwd) = (app.session.id.clone(), app.cwd.clone());
        app.session.close();
        let resumed = Session::open(Options {
            cwd,
            mode: tuse_core::Mode::Tui,
            resume: Some(sid),
            start: Default::default(),
            flags: Default::default(),
        })
        .unwrap();
        let mut app2 = App::new(resumed);
        assert!(!view(&mut app2).contains("row0"));
        let mut stored = false;
        app2.session.with_records(|recs| {
            stored = recs.iter().any(|r| {
                matches!(r, Record::Lua { diffs, .. } if diffs.iter().any(|d| d.diff.contains("row19")))
            });
        });
        assert!(
            stored
                && app2
                    .session
                    .history()
                    .iter()
                    .any(|t| t.detail.contains("row19"))
        );
        crate::ui::app::cleanup(app2);
    }
}
