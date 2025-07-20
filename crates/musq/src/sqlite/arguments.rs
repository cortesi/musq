use crate::{Error, encode::Encode, sqlite::statement::StatementHandle};
use std::collections::HashMap;

use atoi::atoi;

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

impl IntoArguments for Arguments {
    fn into_arguments(self) -> Arguments {
        self
    }
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

    pub(super) fn bind(&self, handle: &mut StatementHandle, offset: usize) -> Result<usize, Error> {
        let mut next_pos = offset;
        if let Some(max) = self.named.values().max().cloned() {
            if max > next_pos {
                next_pos = max;
            }
        }
        // Track mappings from parameter names to argument indices so that multiple
        // references to the same name are bound from the same argument.
        let mut names: HashMap<String, usize> = self.named.clone();

        let cnt = handle.bind_parameter_count();

        for param_i in 1..=cnt {
            // figure out the index of this bind parameter into our argument tuple
            let n: usize = if let Some(name) = handle.bind_parameter_name(param_i) {
                if let Some(rest) = name.strip_prefix('?') {
                    // parameter should have the form ?NNN

                    atoi(rest.as_bytes()).expect("parameter of the form ?NNN")
                } else if let Some(rest) = name.strip_prefix('$') {
                    // parameters of the form $NNN are positional, otherwise they are named
                    if let Some(n) = atoi(rest.as_bytes()) {
                        n
                    } else {
                        *names.entry(rest.to_string()).or_insert_with(|| {
                            next_pos += 1;
                            next_pos
                        })
                    }
                } else if let Some(rest) = name.strip_prefix(':') {
                    *names.entry(rest.to_string()).or_insert_with(|| {
                        next_pos += 1;
                        next_pos
                    })
                } else if let Some(rest) = name.strip_prefix('@') {
                    *names.entry(rest.to_string()).or_insert_with(|| {
                        next_pos += 1;
                        next_pos
                    })
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
    fn bind(&self, handle: &mut StatementHandle, i: usize) -> Result<(), Error> {
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

pub trait IntoArguments: Sized + Send {
    fn into_arguments(self) -> Arguments;
}
