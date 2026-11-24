pub(crate) fn latin(c: char) -> Option<char> {
    let c = c.to_lowercase().next()?;
    "йцукенгшщзфывапролдячсмить"
        .chars()
        .zip("qwertyuiopasdfghjklzxcvbnm".chars())
        .find_map(|(ru, en)| (c == ru).then_some(en))
}
