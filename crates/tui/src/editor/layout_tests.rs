use super::*;

fn ed(s: &str) -> Editor {
    let mut e = Editor::default();
    e.insert(s);
    e
}

#[test]
fn wraps_at_words_and_cuts_long_ones() {
    let e = ed("hello big world");
    assert_eq!(e.layout(8).0, ["hello ", "big ", "world"]);
    assert_eq!(ed("abcdefghij").layout(4).0, ["abcd", "efgh", "ij"]);
    assert_eq!(ed("ab cdefghij").layout(4).0, ["ab ", "cdef", "ghij", ""]);
    // the cursor after a full row gets a row of its own
    assert_eq!(ed("ab cdefghi").layout(4).0, ["ab ", "cdef", "ghi"]);
}

#[test]
fn cursor_follows_wrapped_rows() {
    let mut e = ed("hello big world");
    assert_eq!(e.layout(8).1, (2, 5));
    e.cursor = 6; // start of "big"
    assert_eq!(e.layout(8).1, (1, 0));
    let mut e = ed("abcd");
    assert_eq!(e.layout(4).1, (1, 0));
    e.cursor = 2;
    assert_eq!(e.layout(4).1, (0, 2));
}

#[test]
fn a_click_lands_on_the_cell_in_wrapped_wide_and_atom_text() {
    let mut e = ed("hello big world");
    e.click(1, 2, 8); // row "big ", column 2
    assert_eq!(e.cursor, 8);
    e.click(1, 40, 8); // past the end of the row: before its trailing space
    assert_eq!(e.cursor, 9);
    e.click(9, 0, 8); // below the text: the end
    assert_eq!(e.cursor, 15);
    let mut e = ed("日本語");
    e.click(0, 3, 10); // the middle of the second wide char
    assert_eq!(e.cursor, 1);
    e.click(0, 4, 10);
    assert_eq!(e.cursor, 2);
    let mut e = ed("a ");
    e.insert_image("/x.png".into());
    e.insert("b");
    e.click(0, 5, 40); // inside the atom's label: before it
    assert_eq!(e.cursor, 2);
    e.click(0, 12, 40);
    assert_eq!(e.cursor, 3);
}

#[test]
fn arrows_walk_visual_rows_not_newlines() {
    let mut e = ed("hello big world");
    assert!(e.vertical(true, 8));
    assert_eq!(e.cursor, 9); // row "big ", column 3 (the space)
    assert!(e.vertical(true, 8));
    assert_eq!(e.cursor, 3);
    assert!(!e.vertical(true, 8));
    assert!(e.vertical(false, 8));
    assert!(e.vertical(false, 8));
    assert!(!e.vertical(false, 8));
    let mut e = ed("ab\n\ncd");
    e.cursor = 1;
    assert!(e.vertical(false, 8));
    assert_eq!(e.cursor, 3);
    assert!(e.vertical(false, 8));
    assert_eq!(e.cursor, 4);
}
