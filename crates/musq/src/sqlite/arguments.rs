use std::collections::HashMap;

use atoi::atoi;

use crate::{
    Error, Result,
    encode::Encode,
    sqlite::{Value, statement::StatementHandle},
};

/// Parse a numeric positional parameter like `?1` into its index.
pub fn parse_question_param(name: &str) -> Result<usize> {
    // The parameter must start with exactly one '?'
    if !name.starts_with('?') || name.as_bytes().get(1) == Some(&b'?') {
        return Err(Error::Protocol(format!(
            "invalid numeric SQL parameter: {name}"
        )));
    }

    let rest = &name[1..];

    // reject if the parameter is empty, contains non-digit characters,
    // or has a leading zero
    if rest.is_empty() || rest.starts_with('0') || !rest.as_bytes().iter().all(u8::is_ascii_digit) {
        return Err(Error::Protocol(format!(
            "invalid numeric SQL parameter: {name}"
        )));
    }

    let num = rest
        .parse::<usize>()
        .map_err(|_| Error::Protocol(format!("invalid numeric SQL parameter: {name}")))?;

    if num == 0 {
        return Err(Error::Protocol(format!(
            "invalid numeric SQL parameter: {name}"
        )));
    }

    Ok(num)
}

/// Collection of bound SQL arguments.
#[derive(Default, Debug, Clone)]
pub struct Arguments {
    /// Ordered positional values.
    pub(crate) values: Vec<Value>,
    /// Mapping from named parameters to their argument indices (1-based).
    pub(crate) named: HashMap<String, usize>,
}

impl Arguments {
    /// Add a positional bind parameter.
    pub fn add<T>(&mut self, value: &T) -> Result<()>
    where
        T: Encode + ?Sized,
    {
        self.values
            .push(value.encode().map_err(crate::Error::Encode)?);
        Ok(())
    }

    /// Add a bind parameter by name. The provided `name` may include the
    /// SQLite prefix (`:`, `@`, or `$`) but it is not required.
    pub fn add_named<T>(&mut self, name: &str, value: &T) -> Result<()>
    where
        T: Encode + ?Sized,
    {
        let name = name.trim_start_matches([':', '@', '$', '?']);
        if let Some(&index) = self.named.get(name) {
            self.values[index - 1] = value.encode().map_err(crate::Error::Encode)?;
        } else {
            self.values
                .push(value.encode().map_err(crate::Error::Encode)?);
            let idx = self.values.len();
            self.named.insert(name.to_string(), idx);
        }
        Ok(())
    }

    /// Bind arguments to a prepared statement, returning the number of bound parameters.
    pub(super) fn bind(&self, handle: &StatementHandle, offset: usize) -> Result<usize> {
        // Anonymous-position counter for `?` parameters and for allocating
        // indices to previously-unseen named parameters. This advances strictly
        // in SQL order and is rebased up to the highest named index that has
        // been encountered so far in this statement. Importantly, numeric
        // parameters like `?1` / `$2` do NOT affect this counter; this matches
        // expected behavior like mapping `SELECT ?1, ?` with a single bind to
        // the same index 1.
        let mut anon_pos = offset;
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
                    // numeric positional like `?1`
                    parse_question_param(&name)?
                } else if let Some(rest) = name.strip_prefix('$') {
                    // parameters of the form $NNN are positional, otherwise they are named
                    if let Some(n) = atoi(rest.as_bytes()) {
                        // numeric positional like `$2`
                        if n == 0 || rest.starts_with('0') {
                            return Err(Error::Protocol(format!(
                                "invalid numeric SQL parameter: {name}"
                            )));
                        }
                        n
                    } else if let Some(&idx) = self.named.get(rest) {
                        // existing named parameter
                        if idx > anon_pos {
                            anon_pos = idx;
                        }
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            anon_pos += 1;
                            anon_pos
                        })
                    }
                } else if let Some(rest) = name.strip_prefix(':') {
                    if let Some(&idx) = self.named.get(rest) {
                        if idx > anon_pos {
                            anon_pos = idx;
                        }
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            anon_pos += 1;
                            anon_pos
                        })
                    }
                } else if let Some(rest) = name.strip_prefix('@') {
                    if let Some(&idx) = self.named.get(rest) {
                        if idx > anon_pos {
                            anon_pos = idx;
                        }
                        idx
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            anon_pos += 1;
                            anon_pos
                        })
                    }
                } else {
                    return Err(Error::Protocol(format!(
                        "unsupported SQL parameter format: {name}"
                    )));
                }
            } else {
                // anonymous `?`
                anon_pos += 1;
                anon_pos
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

        Ok(anon_pos - offset)
    }
}

#[cfg(test)]
mod tests {
    use super::parse_question_param;
    use crate::{Arguments, Connection, Error, Musq, query, query_as};

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

    #[test]
    fn test_parse_question_param_trailing_chars() {
        let err = parse_question_param("?12a").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("?12a")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_leading_zero() {
        let err = parse_question_param("?01").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("?01")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_overflow() {
        let big = (usize::MAX as u128 + 1).to_string();
        let param = format!("?{big}");
        let err = parse_question_param(&param).unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains(&param)),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_double_question() {
        let err = parse_question_param("??1").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("??1")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_colon_like() {
        let err = parse_question_param(":1a").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains(":1a")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_at_zero() {
        let err = parse_question_param("@0").unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains("@0")),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[test]
    fn test_parse_question_param_extremely_long() {
        let digits = "1".repeat(4096);
        let param = format!("?{digits}");
        let err = parse_question_param(&param).unwrap_err();
        match err {
            Error::Protocol(msg) => assert!(msg.contains(&param)),
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_parse_dollar_param_zero() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;

        let res = query_as::<(i32,)>("SELECT $0")
            .bind(5_i32)
            .fetch_one(&conn)
            .await;

        assert!(res.is_err());
        match res.unwrap_err() {
            Error::Protocol(msg) => assert!(msg.contains("$0")),
            other => panic!("unexpected error: {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_parse_dollar_param_leading_zero() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;

        let res = query_as::<(i32,)>("SELECT $00")
            .bind(5_i32)
            .fetch_one(&conn)
            .await;

        assert!(res.is_err());
        match res.unwrap_err() {
            Error::Protocol(msg) => assert!(msg.contains("$00")),
            other => panic!("unexpected error: {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_bind_positional_and_named_parameters() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;

        let (a, b, c): (i32, i32, i32) = query_as("SELECT ?1, :b, :a")
            .bind(7_i32)
            .bind_named("b", 8_i32)
            .bind_named("a", 9_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!((a, b, c), (7, 8, 9));

        Ok(())
    }

    #[tokio::test]
    async fn test_error_on_missing_parameter() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;

        let res = query("select ?1, ?2").bind(5_i32).fetch_one(&conn).await;

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
        let conn = Connection::connect_with(&Musq::new()).await?;

        let (v,): (i32,) = query_as("SELECT ?1")
            .bind(5_i32)
            .bind(15_i32)
            .fetch_one(&conn)
            .await?;

        assert_eq!(v, 5);

        Ok(())
    }

    #[tokio::test]
    async fn test_repeated_named_parameters_add_and_named() -> anyhow::Result<()> {
        let conn = Connection::connect_with(&Musq::new()).await?;
        let stmt = conn.prepare("SELECT :a, :a, ?2").await?;
        let mut args = Arguments::default();
        args.add_named("a", &7_i32)?;
        args.add(&9_i32)?;

        let (x, y, z): (i32, i32, i32) = stmt.query_as_with(args).fetch_one(&conn).await?;

        assert_eq!((x, y, z), (7, 7, 9));

        Ok(())
    }
}
