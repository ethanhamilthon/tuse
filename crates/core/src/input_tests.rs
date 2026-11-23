use super::*;

fn tmp(name: &str, bytes: &[u8]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("tuse-input-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

#[test]
fn prompts_are_deduplicated_and_text_stays_clean() {
    let refs = Refs {
        prompts: vec![
            ("a".into(), "one\nline  \n".into()),
            ("b\"".into(), "two".into()),
            ("a".into(), "again".into()),
        ],
        ..Refs::default()
    };
    let i = build("#a #b #a", refs).unwrap();
    assert_eq!(i.text, "#a #b #a");
    assert_eq!(
        i.context,
        "<prompt name=\"a\">one\nline</prompt>\n<prompt name=\"b&quot;\">two</prompt>"
    );
}

#[test]
fn images_keep_ids_and_get_the_next_free_one_from_files() {
    let a = tmp("a.png", b"PNG");
    let b = tmp("b.JPG", b"JPG");
    let refs = Refs {
        images: vec![(3, a.clone())],
        files: vec![b.clone(), b.clone()],
        ..Refs::default()
    };
    let i = build("[Image #3]", refs).unwrap();
    assert_eq!(i.images.len(), 2);
    assert!(i.images[0].starts_with("data:image/png;base64,"));
    assert!(i.images[1].starts_with("data:image/jpeg;base64,"));
    assert!(
        i.context
            .contains(&format!("<image id=\"3\" path=\"{}\"/>", a.display()))
    );
    assert!(
        i.context
            .contains(&format!("<image id=\"4\" path=\"{}\"/>", b.display()))
    );
}

#[test]
fn text_files_are_cut_on_a_utf8_boundary() {
    let small = tmp("s.txt", b"hi\n");
    let exact = tmp("e.txt", "x".repeat(FILE_LIMIT).as_bytes());
    // 1023 ASCII bytes then a 2-byte char straddling the limit.
    let big = tmp(
        "b.txt",
        format!("{}é tail", "x".repeat(FILE_LIMIT - 1)).as_bytes(),
    );
    let refs = |p: &PathBuf| Refs {
        files: vec![p.clone()],
        ..Refs::default()
    };
    let c = build("", refs(&small)).unwrap().context;
    assert_eq!(c, format!("<file path=\"{}\">hi\n</file>", small.display()));
    assert!(
        !build("", refs(&exact))
            .unwrap()
            .context
            .contains("truncated")
    );
    let c = build("", refs(&big)).unwrap().context;
    assert!(c.contains("truncated=\"true\" size=\"1030\""));
    assert!(c.ends_with(&format!("{}</file>", "x".repeat(FILE_LIMIT - 1))));
}

#[test]
fn binary_files_have_no_content_and_missing_ones_follow_strict() {
    let nul = tmp("n.dat", b"ab\0cd");
    let bad = tmp("bad.txt", &[0xff, 0xfe, b'a']);
    for p in [&nul, &bad] {
        let c = build(
            "",
            Refs {
                files: vec![p.clone()],
                ..Refs::default()
            },
        )
        .unwrap()
        .context;
        assert!(c.contains("binary=\"true\"") && c.ends_with("/>"), "{c}");
    }
    let gone = PathBuf::from("/definitely/missing.txt");
    let lax = Refs {
        files: vec![gone.clone()],
        ..Refs::default()
    };
    assert_eq!(build("x", lax).unwrap().context, "");
    let dir = tmp("listed", b"").with_extension("d");
    std::fs::create_dir_all(dir.join("sub")).unwrap();
    std::fs::write(dir.join("a.txt"), "a").unwrap();
    let refs = Refs {
        files: vec![dir.clone()],
        ..Refs::default()
    };
    let shown = dir.display();
    assert_eq!(
        build("x", refs).unwrap().context,
        format!("<dir path=\"{shown}\">\na.txt\nsub/\n</dir>")
    );
    let strict = Refs {
        files: vec![gone],
        strict: true,
        ..Refs::default()
    };
    assert!(build("x", strict).is_err());
}
