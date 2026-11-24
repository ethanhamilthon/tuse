use serde::de::{Deserialize, Deserializer, Error};

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
