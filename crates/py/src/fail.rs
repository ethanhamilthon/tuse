use monty_types::{CallArgs, ExcType, ExtFunctionResult, MontyException, ObjectRef};

/// A Python exception a host function raises.
#[derive(Debug)]
pub struct Fail(pub ExcType, pub String);

impl Fail {
    pub fn value(msg: impl Into<String>) -> Self {
        Fail(ExcType::ValueError, msg.into())
    }

    pub fn of(kind: ExcType, msg: impl Into<String>) -> Self {
        Fail(kind, msg.into())
    }

    pub fn runtime(msg: impl Into<String>) -> Self {
        Fail(ExcType::RuntimeError, msg.into())
    }

    pub fn exception(self) -> MontyException {
        MontyException::new(self.0, Some(self.1))
    }
}

impl From<Fail> for ExtFunctionResult {
    fn from(f: Fail) -> Self {
        ExtFunctionResult::Error(f.exception())
    }
}

/// Typed access to the arguments of a host function call; `None` counts as absent.
pub struct Args<'a>(pub &'a CallArgs);

impl Args<'_> {
    /// A `TypeError` when the call passes `name` (by position `i` or by keyword).
    pub fn deny(&self, i: usize, name: &str) -> Result<(), Fail> {
        if self.0.arg(i).is_some() || self.0.kwarg(name).is_some() {
            return Err(Fail(
                ExcType::TypeError,
                format!("unexpected argument `{name}`"),
            ));
        }
        Ok(())
    }

    fn get(&self, i: usize, name: &str) -> Option<ObjectRef<'_>> {
        self.0
            .arg(i)
            .or_else(|| self.0.kwarg(name))
            .filter(|v| v.type_name() != "NoneType")
    }

    pub fn str(&self, i: usize, name: &str) -> Result<String, Fail> {
        self.opt_str(i, name)?
            .ok_or_else(|| Fail(ExcType::TypeError, format!("missing argument `{name}`")))
    }

    pub fn opt_str(&self, i: usize, name: &str) -> Result<Option<String>, Fail> {
        match self.get(i, name) {
            None => Ok(None),
            Some(v) => v.as_str().map(|s| Some(s.to_string())).ok_or_else(|| {
                Fail(
                    ExcType::TypeError,
                    format!("`{name}` must be str, not {}", v.type_name()),
                )
            }),
        }
    }

    pub fn opt_int(&self, i: usize, name: &str) -> Result<Option<i64>, Fail> {
        match self.get(i, name) {
            None => Ok(None),
            Some(v) => v.as_int().map(Some).ok_or_else(|| {
                Fail(
                    ExcType::TypeError,
                    format!("`{name}` must be int, not {}", v.type_name()),
                )
            }),
        }
    }

    pub fn bool(&self, i: usize, name: &str, default: bool) -> Result<bool, Fail> {
        match self.get(i, name) {
            None => Ok(default),
            Some(v) => v.as_bool().ok_or_else(|| {
                Fail(
                    ExcType::TypeError,
                    format!("`{name}` must be bool, not {}", v.type_name()),
                )
            }),
        }
    }

    pub fn str_list(&self, i: usize, name: &str) -> Result<Vec<String>, Fail> {
        let bad = || {
            Fail(
                ExcType::TypeError,
                format!("`{name}` must be a list of str"),
            )
        };
        let Some(v) = self.get(i, name) else {
            return Ok(vec![]);
        };
        v.items()
            .ok_or_else(bad)?
            .iter()
            .map(|x| x.as_str().map(String::from).ok_or_else(bad))
            .collect()
    }

    pub fn str_map(&self, i: usize, name: &str) -> Result<Vec<(String, String)>, Fail> {
        let bad = || {
            Fail(
                ExcType::TypeError,
                format!("`{name}` must be a dict of str"),
            )
        };
        let Some(v) = self.get(i, name) else {
            return Ok(vec![]);
        };
        v.pairs()
            .ok_or_else(bad)?
            .iter()
            .map(|(k, v)| match (k.as_str(), v.as_str()) {
                (Some(k), Some(v)) => Ok((k.to_string(), v.to_string())),
                _ => Err(bad()),
            })
            .collect()
    }
}
