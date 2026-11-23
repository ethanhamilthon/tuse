use serde::de::{Deserialize, Deserializer, Error};
use std::collections::BTreeMap;

/// A list field: Lua's `{}` arrives as an empty map, so that is an empty list too.
pub fn list<'de, D: Deserializer<'de>, T: Deserialize<'de>>(d: D) -> Result<Vec<T>, D::Error> {
    struct V<T>(std::marker::PhantomData<T>);
    impl<'de, T: Deserialize<'de>> serde::de::Visitor<'de> for V<T> {
        type Value = Vec<T>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a list")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
            let mut out = vec![];
            while let Some(x) = seq.next_element()? {
                out.push(x);
            }
            Ok(out)
        }
        fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<Vec<T>, A::Error> {
            match map.next_key::<String>()? {
                None => Ok(vec![]),
                Some(k) => Err(A::Error::custom(format!("expected a list, got key `{k}`"))),
            }
        }
    }
    d.deserialize_any(V(std::marker::PhantomData))
}

/// An integer in `MIN..=MAX`.
pub fn int<'de, D, T, const MIN: i64, const MAX: i64>(d: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: TryFrom<i64>,
{
    let n = i64::deserialize(d)?;
    if !(MIN..=MAX).contains(&n) {
        return Err(D::Error::custom(format!(
            "value {n} out of range ({MIN}..={MAX})"
        )));
    }
    T::try_from(n).map_err(|_| D::Error::custom(format!("value {n} out of range")))
}

pub fn opt_u64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<u64>, D::Error> {
    int::<D, u64, 0, { i64::MAX }>(d).map(Some)
}

/// `-1` (no limit) or at least `1`.
pub fn rows<'de, D: Deserializer<'de>>(d: D) -> Result<i32, D::Error> {
    let n = int::<D, i32, -1, { i32::MAX as i64 }>(d)?;
    if n == 0 {
        return Err(D::Error::custom("must be -1 or >= 1"));
    }
    Ok(n)
}

pub fn price<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
    let n = f64::deserialize(d)?;
    if !n.is_finite() || n < 0.0 {
        return Err(D::Error::custom("expected a finite non-negative number"));
    }
    Ok(n)
}

pub fn color<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    if !crate::theme::valid_color(&s) {
        return Err(D::Error::custom(format!("invalid color `{s}`")));
    }
    Ok(s)
}

pub fn model_name<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    let (provider, id) = crate::split_model(&s);
    if provider.is_empty() || id.is_empty() || s.chars().any(char::is_whitespace) {
        return Err(D::Error::custom(format!(
            "expected a provider/model name, got `{s}`"
        )));
    }
    Ok(s)
}

pub fn non_empty<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let s = String::deserialize(d)?;
    if s.trim().is_empty() {
        return Err(D::Error::custom("must not be empty"));
    }
    Ok(s)
}

pub fn events<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    if list.is_empty() {
        return Err(D::Error::custom("must contain at least one event"));
    }
    if let Some(e) = list
        .iter()
        .find(|e| !crate::HOOK_EVENTS.contains(&e.as_str()))
    {
        return Err(D::Error::custom(format!(
            "unknown event `{e}`; events are {}",
            crate::HOOK_EVENTS.join(", ")
        )));
    }
    Ok(list)
}

fn names<E: Error>(list: &[String], check: fn(&str) -> Option<&'static str>) -> Result<(), E> {
    match list.iter().find_map(|n| check(n).map(|r| (n, r))) {
        Some((n, reason)) => Err(E::custom(format!("`{n}`: {reason}"))),
        None => Ok(()),
    }
}

pub fn functions<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<String, crate::Function>, D::Error> {
    let map = BTreeMap::<String, crate::Function>::deserialize(d)?;
    names(
        &map.keys().cloned().collect::<Vec<_>>(),
        crate::check_function_name,
    )?;
    Ok(map)
}

pub fn commands<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    names(&list, crate::check_command_name)?;
    Ok(list)
}

pub fn open<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    let list: Vec<String> = list(d)?;
    names(&list, crate::check_open_ext)?;
    Ok(list)
}
