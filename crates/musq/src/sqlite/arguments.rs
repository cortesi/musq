use crate::{
    Error, Result,
    encode::Encode,
    sqlite::{Value, statement::StatementHandle},
};
use std::collections::HashMap;

use atoi::atoi;

pub(crate) fn parse_question_param(name: &str) -> Result<usize> {
    let rest = name.trim_start_matches('?');
    let num = atoi::<usize>(rest.as_bytes())
        .ok_or_else(|| Error::Protocol(format!("invalid numeric SQL parameter: {name}")))?;

    if num == 0 {
        return Err(Error::Protocol(format!(
            "invalid numeric SQL parameter: {name}"
        )));
    }

    Ok(num)
}

#[derive(Default, Debug)]
pub struct Arguments {
    pub(crate) values: Vec<Value>,
    /// Mapping from named parameters to their argument indices (1-based).
    pub(crate) named: HashMap<String, usize>,
}

impl Arguments {
    pub fn add<T>(&mut self, value: T)
    where
        T: Encode,
    {
        self.values.push(value.encode());
    }

    /// Add a bind parameter by name. The provided `name` may include the
    /// SQLite prefix (`:`, `@`, or `$`) but it is not required.
    pub fn add_named<T>(&mut self, name: &str, value: T)
    where
        T: Encode,
    {
        let name = name.trim_start_matches([':', '@', '$', '?']);
        if let Some(&index) = self.named.get(name) {
            self.values[index - 1] = value.encode();
        } else {
            self.values.push(value.encode());
            let idx = self.values.len();
            self.named.insert(name.to_string(), idx);
        }
    }

    pub(super) fn bind(&self, handle: &mut StatementHandle, offset: usize) -> Result<usize> {
        let mut next_pos = offset;
        if let Some(max) = self.named.values().max().cloned() {
            if max > next_pos {
                next_pos = max;
            }
        }
        // Track mappings from positional-parameter-introduced names to their
        // argument indices so that multiple references to the same name are
        // bound from the same argument. We first consult `self.named` for
        // values explicitly bound via [`add_named`].
        let mut names: HashMap<String, usize> = HashMap::new();

        let cnt = handle.bind_parameter_count();

        for param_i in 1..=cnt {
            // Figure out the index of this bind parameter into our argument tuple.
            let n: usize = if let Some(name) = handle.bind_parameter_name(param_i) {
                if name.starts_with('?') {
                    parse_question_param(&name)?
                } else if let Some(rest) = name.strip_prefix('$') {
                    // parameters of the form $NNN are positional, otherwise they are named
                    if let Some(n) = atoi(rest.as_bytes()) {
                        n
                    } else if let Some(&idx) = self.named.get(rest) {
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            next_pos += 1;
                            next_pos
                        })
                    }
                } else if let Some(rest) = name.strip_prefix(':') {
                    if let Some(&idx) = self.named.get(rest) {
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            next_pos += 1;
                            next_pos
                        })
                    }
                } else if let Some(rest) = name.strip_prefix('@') {
                    if let Some(&idx) = self.named.get(rest) {
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            next_pos += 1;
                            next_pos
                        })
                    }
                } else {
                    return Err(Error::Protocol(format!(
                        "unsupported SQL parameter format: {name}"
                    )));
                }
            } else {
                next_pos += 1;
                next_pos
            };

            if n > self.values.len() {
                return Err(Error::Protocol(format!(
                    "bind parameter index out of bounds: the len is {}, but the index is {}",
                    self.values.len(),
                    n
                )));
            }

            self.values[n - 1].bind(handle, param_i)?;
        }

        Ok(next_pos - offset)
    }
}

impl Value {
    /// Bind this value to the parameter `i` of the given statement handle.
    ///
    /// The binding is performed according to the underlying variant without
    /// altering the stored value.
    pub(crate) fn bind(&self, handle: &mut StatementHandle, i: usize) -> Result<()> {
        match self {
            Value::Text { value, .. } => {
                handle.bind_text(i, std::str::from_utf8(value).unwrap())?
            }
            Value::Blob { value, .. } => handle.bind_blob(i, value.as_slice())?,
            Value::Integer { value, .. } => handle.bind_int64(i, *value)?,
            Value::Double { value, .. } => handle.bind_double(i, *value)?,
            Value::Null { .. } => handle.bind_null(i)?,
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_question_param;
    use crate::Error;

    #[test]
    fn test_parse_question_param_invalid() {
        let err = parse_question_param("?foo").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("?foo")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_zero() {
        let err = parse_question_param("?0").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("?0")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
