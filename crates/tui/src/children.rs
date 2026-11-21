use crate::app::App;
use crate::panel::Panel;
use crate::picker::{Kind, Picker, item};
use crate::render::duration;
use std::time::Duration;
use tuse_core::{Child, Totals, tokens};

const NAME_MAX: usize = 28;

fn spend(t: &Totals) -> String {
    t.cost.map_or(String::new(), |c| format!(" ${c:.2}"))
}

/// Input plus output tokens and the spend, as short as it gets.
fn short(t: &Totals) -> String {
    format!("{}{}", tokens(t.usage.input + t.usage.output), spend(t))
}

/// A compact row: indent and name on the left, status and the main numbers after it.
pub fn row(c: &Child) -> (String, String) {
    let name = crate::text::clip(&c.name, NAME_MAX, crate::style::MORE);
    let status = if c.running { "running" } else { "done" };
    let mut detail = format!(
        "{status} {} {}",
        duration(Duration::from_secs(c.elapsed_secs)),
        short(&c.own)
    );
    if c.has_children {
        detail.push_str(&format!(" | all {}", short(&c.subtree)));
    }
    (format!("{}{name}", "  ".repeat(c.depth)), detail)
}

fn usage(t: &Totals) -> String {
    let u = &t.usage;
    let cost = t.cost.map_or("no price".into(), |c| format!("${c:.4}"));
    format!(
        "in {} out {} cached {} spend {cost}",
        u.input, u.output, u.cached
    )
}

/// Every field of one row, for the detail view.
pub fn detail(c: &Child) -> String {
    let window = c.window.map_or("?".into(), |w| w.to_string());
    let mut s = format!(
        "id       {}\nname     {}\nstatus   {}\ncwd      {}\nelapsed  {}\ncontext  {} / {window}\ntokens   {}",
        c.id,
        c.name,
        if c.running { "running" } else { "done" },
        c.cwd,
        duration(Duration::from_secs(c.elapsed_secs)),
        c.context,
        usage(&c.own),
    );
    if c.has_children {
        s.push_str(&format!("\nsubtree  {}", usage(&c.subtree)));
    }
    s
}

impl App {
    pub fn open_children(&mut self, select: Option<&str>) {
        let list = self.session.children();
        if list.is_empty() {
            self.close_modal();
            self.flash("No child sessions. Tuse processes started with TUSE_PARENT show up here.");
            return;
        }
        let items = list
            .iter()
            .map(|c| {
                let (label, detail) = row(c);
                item(label, detail, &c.id)
            })
            .collect();
        let p = Picker::new(Kind::Children, "Child sessions (enter: details)", items);
        let p = self.open_modal(p);
        if let Some(v) = select {
            p.select(v);
        }
    }

    pub fn open_child_detail(&mut self, id: &str) {
        let Some(c) = self.session.children().into_iter().find(|c| c.id == id) else {
            return self.open_children(None);
        };
        self.children_view = Some(id.to_string());
        let title = crate::text::clip(&c.name, NAME_MAX, crate::style::MORE);
        self.panel = Some(Panel::new(format!("Session {title}"), detail(&c)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn own() -> Totals {
        let mut t = Totals {
            cost: Some(12.3456),
            ..Default::default()
        };
        (t.usage.input, t.usage.output, t.usage.cached) = (123_456, 7_890, 100_000);
        t
    }

    fn child() -> Child {
        Child {
            id: "20261121T000000.000000000Z-0123456789abcdef".into(),
            name: "a rather long session title that must be clipped".into(),
            depth: 2,
            running: true,
            cwd: "/Users/someone/projects/a/very/deep/path/that/is/long".into(),
            elapsed_secs: 3725,
            own: own(),
            has_children: true,
            context: 131_346,
            window: Some(200_000),
            ..Default::default()
        }
    }

    #[test]
    fn long_titles_wrap_in_the_detail_panel() {
        let p = crate::style::Palette::new(&Default::default(), &Default::default());
        let mut c = child();
        c.name = "x".repeat(70) + " should_not_hide_the_end";
        let mut panel = Panel::new("Session", detail(&c));
        let text: String = panel
            .lines(40, 20, &p)
            .iter()
            .map(|l| l.iter().map(|s| s.text.as_str()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n");
        let flat = text.replace(['\n', ' '], "");
        assert!(flat.contains(&c.name.replace(' ', "")), "{text}");
    }

    #[test]
    fn rows_fit_100_columns_and_details_show_every_field() {
        let mut c = child();
        c.subtree = c.own;
        let (label, detail_col) = row(&c);
        let w = crate::text::width(&label) + 2 + crate::text::width(&detail_col) + 4;
        assert!(w <= 100, "{w}: {label} {detail_col}");
        assert!(detail_col.contains("running") && detail_col.contains("$12.35"));
        let d = detail(&c);
        for want in [
            "0123456789abcdef",
            "/very/deep/path",
            "1h2m",
            "cached 100000",
            "131346 / 200000",
            "$12.3456",
            "subtree",
            "must be clipped",
        ] {
            assert!(d.contains(want), "{want} missing in {d}");
        }
    }
}
