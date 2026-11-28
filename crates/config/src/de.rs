use serde::de::{Deserialize, Deserializer, Error};

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
