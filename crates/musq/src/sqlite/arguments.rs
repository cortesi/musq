use crate::{Error, Result, encode::Encode, sqlite::statement::StatementHandle};
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

#[derive(Debug)]
pub enum ArgumentValue {
    Null,
    Text(String),
    Blob(Vec<u8>),
    Double(f64),
    Int(i32),
    Int64(i64),
}

#[derive(Default, Debug)]
pub struct Arguments {
    pub(crate) values: Vec<ArgumentValue>,
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

impl ArgumentValue {
    fn bind(&self, handle: &mut StatementHandle, i: usize) -> Result<()> {
        use ArgumentValue::*;

        match self {
            Text(v) => handle.bind_text(i, v.as_str())?,
            Blob(v) => handle.bind_blob(i, v.as_slice())?,
            Int(v) => handle.bind_int(i, *v)?,
            Int64(v) => handle.bind_int64(i, *v)?,
            Double(v) => handle.bind_double(i, *v)?,
            Null => handle.bind_null(i)?,
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_question_param;
    use crate::{Connection, Error, Musq, query, query_as};

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

    #[tokio::test]
    async fn test_bind_positional_and_named_parameters() -> anyhow::Result<()> {
        let mut conn = Connection::connect_with(&Musq::new()).await?;

        let (a, b, c): (i32, i32, i32) = query_as("SELECT ?1, :b, :a")
            .bind(7_i32)
            .bind_named("b", 8_i32)
            .bind_named("a", 9_i32)
            .fetch_one(&mut conn)
            .await?;

        assert_eq!((a, b, c), (7, 8, 9));

        Ok(())
    }

    #[tokio::test]
    async fn test_error_on_missing_parameter() -> anyhow::Result<()> {
        let mut conn = Connection::connect_with(&Musq::new()).await?;

        let res = query("select ?1, ?2")
            .bind(5_i32)
            .fetch_one(&mut conn)
            .await;

        assert!(res.is_err());

        if let Err(Error::Protocol(msg)) = res {
            assert!(msg.contains("index is 2"));
        } else {
            panic!("expected protocol error");
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_excess_parameters_are_ignored() -> anyhow::Result<()> {
        let mut conn = Connection::connect_with(&Musq::new()).await?;

        let (v,): (i32,) = query_as("SELECT ?1")
            .bind(5_i32)
            .bind(15_i32)
            .fetch_one(&mut conn)
            .await?;

        assert_eq!(v, 5);

        Ok(())
    }
}
