mod common;

use std::fs;
use std::path::Path;
use std::process::Command;
use tuse_core::{Mode, Options, Session, Start};

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.com")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.com")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?} in {} failed:\nstdout: {}\nstderr: {}",
        args,
        dir.display(),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn make_remote(bare: &Path, work: &Path, branch: &str, body: &str) -> String {
    fs::create_dir_all(bare).unwrap();
    fs::create_dir_all(work).unwrap();
    git(bare, &["init", "--bare", "-b", branch]);
    git(work, &["init", "-b", branch]);
    git(work, &["remote", "add", "origin", &bare.to_string_lossy()]);
    fs::write(work.join("init.lua"), body).unwrap();
    fs::create_dir_all(work.join("plugins/sub")).unwrap();
    fs::write(
        work.join("plugins/sub/init.lua"),
        "return { prompts = { sub = 'sub-v1' } }",
    )
    .unwrap();
    git(work, &["add", "."]);
    git(work, &["commit", "-m", "init"]);
    git(work, &["push", "-u", "origin", branch]);
    git(work, &["rev-parse", "HEAD"])
}

fn commit_work(work: &Path, file: &str, content: &str, msg: &str) -> String {
    let p = work.join(file);
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(p, content).unwrap();
    git(work, &["add", file]);
    git(work, &["commit", "-m", msg]);
    git(work, &["rev-parse", "HEAD"])
}

#[test]
fn reload_installs_updates_pins_protects_local_work_and_rolls_back() {
    let addr = common::provider(vec![]);
    let (dir, cfg) = common::setup("plugin-sync", &addr, "");
    let remotes = dir.join("remotes");
    let home = dir.join("home");
    fs::write(
        home.join(".gitconfig"),
        format!(
            "[url \"file://{}/\"]\n\tinsteadOf = https://github.com/\n",
            remotes.display()
        ),
    )
    .unwrap();

    let bare1 = remotes.join("me/kit.git");
    let work1 = dir.join("work1");
    let sha1 = make_remote(
        &bare1,
        &work1,
        "master",
        "return { prompts = { kit = 'v1' } }",
    );

    let bare2 = remotes.join("me/urlrepo.git");
    let work2 = dir.join("work2");
    let _ = make_remote(
        &bare2,
        &work2,
        "main",
        "return { prompts = { url = 'u1' } }",
    );
    let url2 = format!("file://{}", bare2.display());

    fs::write(&cfg, "return { plugins = { 'me/kit' } }").unwrap();
    let offline_err = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .err()
    .unwrap();
    assert!(offline_err.contains("not checked out"), "{offline_err}");

    let local_dir = dir.join("local-plug");
    fs::create_dir_all(&local_dir).unwrap();
    fs::write(
        local_dir.join("init.lua"),
        "return { prompts = { loc = 'l1' } }",
    )
    .unwrap();

    fs::write(&cfg, "return { prompts = { base = 'ok' } }").unwrap();
    let s = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();

    fs::write(
        &cfg,
        format!(
            "return {{ plugins = {{ 'me/kit', 'me/kit/plugins/sub', '{url2}', {{ 'loc', dir = 'local-plug' }}, {{ 'me/off', enabled = false }} }} }}"
        ),
    )
    .unwrap();
    s.reload().unwrap();
    let c = s.config();
    assert_eq!(c.prompts["kit"], "v1");
    assert_eq!(c.prompts["sub"], "sub-v1");
    assert_eq!(c.prompts["url"], "u1");
    assert_eq!(c.prompts["loc"], "l1");
    assert_eq!(c.plugins[0].commit.as_deref(), Some(&sha1[..7]));

    fs::write(
        local_dir.join("init.lua"),
        "return { prompts = { loc = 'l2' } }",
    )
    .unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["loc"], "l2");

    fs::write(
        &cfg,
        "return { plugins = { { 'me/missing-repo', ref = 'nope' } } }",
    )
    .unwrap();
    assert!(s.reload().is_err());
    assert!(!home.join(".tuse/plugins/me/missing-repo").exists());
    assert_eq!(s.config().prompts["kit"], "v1");

    let sha2 = commit_work(
        &work1,
        "init.lua",
        "return { prompts = { kit = 'v2' } }",
        "v2",
    );
    git(&work1, &["push", "origin", "master"]);
    git(&work1, &["checkout", "-b", "main"]);
    let sha3 = commit_work(
        &work1,
        "init.lua",
        "return { prompts = { kit = 'v3-main' } }",
        "main",
    );
    git(&work1, &["push", "-u", "origin", "main"]);
    git(&bare1, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    git(&work1, &["push", "origin", ":master"]);

    fs::write(&cfg, "return { plugins = { 'me/kit' } }").unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v3-main");
    assert_eq!(s.config().plugins[0].commit.as_deref(), Some(&sha3[..7]));

    git(&work1, &["tag", "rel", &sha2]);
    git(&work1, &["push", "origin", "rel"]);
    fs::write(&cfg, "return { plugins = { { 'me/kit', ref = 'rel' } } }").unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v2");

    git(&work1, &["tag", "-f", "rel", &sha3]);
    git(&work1, &["push", "-f", "origin", "rel"]);
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v3-main");

    git(&work1, &["checkout", "-b", "feat"]);
    commit_work(
        &work1,
        "init.lua",
        "return { prompts = { kit = 'feat' } }",
        "feat",
    );
    git(&work1, &["push", "-u", "origin", "feat"]);
    fs::write(&cfg, "return { plugins = { { 'me/kit', ref = 'feat' } } }").unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "feat");
    git(&work1, &["checkout", "main"]);
    git(&work1, &["merge", "--no-ff", "feat", "-m", "merge feat"]);
    git(&work1, &["push", "origin", "main"]);
    git(&work1, &["push", "origin", ":feat"]);
    let del_err = s.reload().unwrap_err();
    assert!(del_err.contains("ref 'feat' not found"), "{del_err}");
    assert_eq!(s.config().prompts["kit"], "feat");

    fs::write(
        &cfg,
        format!("return {{ plugins = {{ {{ 'me/kit', ref = '{sha2}' }} }} }}"),
    )
    .unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v2");
    let bare1_bak = remotes.join("me/kit-bak.git");
    fs::rename(&bare1, &bare1_bak).unwrap();
    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v2");
    fs::rename(&bare1_bak, &bare1).unwrap();

    fs::write(
        &cfg,
        "return { plugins = { 'https://github.com/a/b.git', 'git@github.com:a/b.git' } }",
    )
    .unwrap();
    let url_err = s.reload().unwrap_err();
    assert!(
        url_err.contains("map to the same checkout with different URLs"),
        "{url_err}"
    );

    let non_git = home.join(".tuse/plugins/me/notgit");
    fs::create_dir_all(&non_git).unwrap();
    fs::write(&cfg, "return { plugins = { 'me/notgit' } }").unwrap();
    let ng_err = s.reload().unwrap_err();
    assert!(ng_err.contains("not a git repository"), "{ng_err}");

    let kit_checkout = home.join(".tuse/plugins/me/kit");
    git(
        &kit_checkout,
        &[
            "remote",
            "set-url",
            "origin",
            "https://example.com/other.git",
        ],
    );
    fs::write(&cfg, "return { plugins = { 'me/kit' } }").unwrap();
    let mm_err = s.reload().unwrap_err();
    assert!(mm_err.contains("checkout remote URL"), "{mm_err}");
    assert_eq!(
        git(&kit_checkout, &["config", "--get", "remote.origin.url"]),
        "https://example.com/other.git"
    );
    git(
        &kit_checkout,
        &[
            "remote",
            "set-url",
            "origin",
            "https://github.com/me/kit.git",
        ],
    );

    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "feat");

    let sha4 = commit_work(
        &work1,
        "init.lua",
        "return { prompts = { kit = 'v4' } }",
        "v4",
    );
    git(&work1, &["push", "origin", "main"]);

    fs::write(
        kit_checkout.join("init.lua"),
        "return { prompts = { kit = 'dirty' } }",
    )
    .unwrap();
    let dirty_err = s.reload().unwrap_err();
    assert!(
        dirty_err.contains("plugin me/kit:") && dirty_err.contains("local changes"),
        "{dirty_err}"
    );
    assert!(
        fs::read_to_string(kit_checkout.join("init.lua"))
            .unwrap()
            .contains("dirty")
    );
    git(&kit_checkout, &["checkout", "--", "init.lua"]);

    fs::write(kit_checkout.join("untracked.txt"), "keep").unwrap();
    assert!(s.reload().unwrap_err().contains("local changes"));
    assert!(kit_checkout.join("untracked.txt").exists());
    fs::remove_file(kit_checkout.join("untracked.txt")).unwrap();

    commit_work(&kit_checkout, "local.txt", "detached", "detached commit");
    let det_sha = git(&kit_checkout, &["rev-parse", "HEAD"]);
    assert!(s.reload().unwrap_err().contains("unpublished commits"));
    assert_eq!(git(&kit_checkout, &["rev-parse", "HEAD"]), det_sha);

    git(&kit_checkout, &["checkout", "-b", "local-br"]);
    assert!(s.reload().unwrap_err().contains("unpublished commits"));
    git(&kit_checkout, &["checkout", "--detach", &sha3]);
    git(&kit_checkout, &["branch", "-D", "local-br"]);

    fs::write(kit_checkout.join(".git/info/exclude"), "clash.txt\n").unwrap();
    fs::write(kit_checkout.join("clash.txt"), "precious").unwrap();
    commit_work(&work1, "clash.txt", "upstream", "add clash");
    git(&work1, &["push", "origin", "main"]);
    let clash_err = s.reload().unwrap_err();
    assert!(clash_err.contains("git checkout failed"), "{clash_err}");
    assert_eq!(
        fs::read_to_string(kit_checkout.join("clash.txt")).unwrap(),
        "precious"
    );
    fs::remove_file(kit_checkout.join("clash.txt")).unwrap();

    s.reload().unwrap();
    assert_eq!(s.config().prompts["kit"], "v4");
    assert_ne!(s.config().plugins[0].commit.as_deref(), Some(&sha4[..7]));

    let s_other = Session::open(Options {
        cwd: dir.clone(),
        config: Some(cfg.clone()),
        mode: Mode::Json,
        resume: None,
        start: Start::Launch,
    })
    .unwrap();
    let t1 = {
        let s1 = s.clone();
        std::thread::spawn(move || s1.reload())
    };
    let t2 = {
        let s2 = s_other.clone();
        std::thread::spawn(move || s2.reload())
    };
    t1.join().unwrap().unwrap();
    t2.join().unwrap().unwrap();
    s_other.close();

    fs::write(&cfg, "return { prompts = { after = 'clean' } }").unwrap();
    s.reload().unwrap();
    assert!(!s.config().prompts.contains_key("kit"));
    assert!(kit_checkout.exists());

    s.close();
    common::cleanup(&dir);
}
