use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tuse_py::{Args, Ctx, Fail, Host, MontyObject, Python};

struct H {
    dir: PathBuf,
}

impl Host for H {
    fn functions(&self) -> &'static [&'static str] {
        &["twice"]
    }
    fn call(&self, name: &str, args: &Args, _: &Ctx) -> Result<MontyObject, Fail> {
        assert_eq!(name, "twice");
        Ok(MontyObject::string(args.str(0, "s")?.repeat(2)))
    }
    fn cwd(&self) -> PathBuf {
        self.dir.clone()
    }
    fn env(&self) -> Vec<(String, String)> {
        vec![("TUSE_SESSION".into(), "s1".into())]
    }
    fn module_dirs(&self) -> Vec<PathBuf> {
        vec![self.dir.clone()]
    }
}

fn run(py: &mut Python, code: &str, secs: u64, cancel: &AtomicBool) -> (String, bool) {
    let ctx = Ctx {
        cancel,
        deadline: Instant::now() + Duration::from_secs(secs),
    };
    let o = py.run(code, &ctx, vec![]);
    (o.text, o.ok)
}

#[test]
fn the_interpreter_works_like_a_repl_with_sh_files_and_limits() {
    let dir = std::env::temp_dir().join(format!("tuse-py-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("helpers.py"), "def hi(n):\n    return 'hi ' + n\n").unwrap();
    let mut py = Python::new(Arc::new(H { dir: dir.clone() }), 1000);
    let no = AtomicBool::new(false);
    let mut go = |code: &str| run(&mut py, code, 5, &no);

    // State lives between calls; print and the last expression are both shown.
    assert_eq!(go("x = 20\nprint('a')"), ("a".into(), true));
    assert_eq!(go("x + 1"), ("21".into(), true));
    // A host function, and sh results with fields.
    assert_eq!(go("twice('ab')").0, "abab");
    let r = go("r = sh('echo out; echo err >&2; exit 3')\nprint(r.ok, r.code, repr(r.out))");
    assert_eq!(r.0, "False 3 'out\\nerr\\n'");
    assert_eq!(go("sh('echo hello')").0, "hello");
    assert_eq!(go("sh('exit 2')").0, "[exit code 2]");
    assert_eq!(
        go("sh('pwd').out.strip()").0,
        dir.canonicalize().unwrap().display().to_string()
    );
    assert_eq!(go("import os\nos.environ['TUSE_SESSION']").0, "s1");
    // Files, relative to the working directory, and an import from that folder.
    assert_eq!(
        go("from pathlib import Path\nPath('f.txt').write_text('data')\nPath('f.txt').read_text()")
            .0,
        "data"
    );
    assert_eq!(std::fs::read_to_string(dir.join("f.txt")).unwrap(), "data");
    assert_eq!(go("import helpers\nhelpers.hi('x')").0, "hi x");
    // Errors keep the interpreter alive.
    let e = go("1/0");
    assert!(!e.1 && e.0.contains("ZeroDivisionError"), "{}", e.0);
    assert_eq!(go("x").0, "20");
    // A pure-Python loop and a slow sh both end at the deadline.
    let t = Instant::now();
    let (text, ok) = run(&mut py, "while True:\n    pass", 1, &no);
    assert!(
        !ok && text.contains("timed out") && t.elapsed() < Duration::from_secs(4),
        "{text}"
    );
    let t = Instant::now();
    let (text, _) = run(&mut py, "sh('sleep 10')", 1, &no);
    assert!(
        text.contains("timed out") && t.elapsed() < Duration::from_secs(4),
        "{text}"
    );
    // Cancel stops both too.
    let flag = AtomicBool::new(false);
    let t = Instant::now();
    let (text, _) = std::thread::scope(|s| {
        s.spawn(|| {
            std::thread::sleep(Duration::from_millis(300));
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        });
        run(&mut py, "sh('sleep 10')", 30, &flag)
    });
    assert!(
        text.contains("cancelled") && t.elapsed() < Duration::from_secs(4),
        "{text}"
    );
    // Long output is cut in the middle.
    flag.store(false, std::sync::atomic::Ordering::Relaxed);
    let (text, _) = run(&mut py, "print('a' * 5000)", 5, &flag);
    assert!(
        text.contains("bytes cut") && text.len() < 1100,
        "{}",
        text.len()
    );
    std::fs::remove_dir_all(dir).ok();
}
