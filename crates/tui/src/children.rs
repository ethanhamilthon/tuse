use crate::app::App;
use crate::picker::{Kind, Picker, item};
use crate::render::duration;
use std::time::Duration;
use tuse_core::{Child, Totals, tokens};

fn spend(t: &Totals) -> String {
    t.cost.map_or(String::new(), |c| format!(" ${c:.4}"))
}

fn usage(t: &Totals) -> String {
    let u = &t.usage;
    format!(
        "in {} out {} cached {}{}",
        tokens(u.input),
        tokens(u.output),
        tokens(u.cached),
        spend(t)
    )
}

/// The detail column of one row.
fn detail(c: &Child) -> String {
    let status = if c.running { "running" } else { "done" };
    let window = c.window.map_or("?".into(), tokens);
    let mut s = format!(
        "{status}  {}  {}  {}  ctx {}/{window}",
        c.cwd,
        duration(Duration::from_secs(c.elapsed_secs)),
        usage(&c.own),
        tokens(c.context),
    );
    if c.has_children {
        s.push_str(&format!("  subtree {}", usage(&c.subtree)));
    }
    s
}

impl App {
    pub fn open_children(&mut self) {
        let list = self.session.children();
        if list.is_empty() {
            self.close_modal();
            self.flash("No child sessions. Tuse processes started with TUSE_PARENT show up here.");
            return;
        }
        let items = list
            .iter()
            .map(|c| {
                item(
                    format!("{}{}", "  ".repeat(c.depth), c.name),
                    detail(c),
                    &c.id,
                )
            })
            .collect();
        let p = Picker::new(Kind::Children, "Child sessions", items);
        self.open_modal(p);
    }
}
