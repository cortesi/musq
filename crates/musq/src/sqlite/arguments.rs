use crate::{Error, encode::Encode, sqlite::statement::StatementHandle};

use atoi::atoi;
use libsqlite3_sys::SQLITE_OK;

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

    pub(super) fn bind(&self, handle: &mut StatementHandle, offset: usize) -> Result<usize, Error> {
        let mut arg_i = offset;
        // for handle in &statement.handles {

        let cnt = handle.bind_parameter_count();

        for param_i in 1..=cnt {
            // figure out the index of this bind parameter into our argument tuple
            let n: usize = if let Some(name) = handle.bind_parameter_name(param_i) {
                if let Some(name) = name.strip_prefix('?') {
                    // parameter should have the form ?NNN
                    atoi(name.as_bytes()).expect("parameter of the form ?NNN")
                } else if let Some(name) = name.strip_prefix('$') {
                    // parameter should have the form $NNN
                    atoi(name.as_bytes()).ok_or_else(|| {
                        Error::Protocol(format!(
                            "parameters with non-integer names are not currently supported: {name}"
                        ))
                    })?
                } else {
                    return Err(Error::Protocol(format!(
                        "unsupported SQL parameter format: {name}"
                    )));
                }
            } else {
                arg_i += 1;
                arg_i
            };

            if n > self.values.len() {
                // SQLite treats unbound variables as NULL we reproduce this here. If you are reading this and think
                // this should be an error, open an issue and we can discuss configuring this somehow. Note that the
                // query macros have a different way of enforcing argument arity.
                break;
            }

            self.values[n - 1].bind(handle, param_i)?;
        }

        Ok(arg_i - offset)
    }
}

impl ArgumentValue {
    fn bind(&self, handle: &mut StatementHandle, i: usize) -> Result<(), Error> {
        use ArgumentValue::*;

        let status = match self {
            Text(v) => handle.bind_text(i, v.as_str()),
            Blob(v) => handle.bind_blob(i, v.as_slice()),
            Int(v) => handle.bind_int(i, *v),
            Int64(v) => handle.bind_int64(i, *v),
            Double(v) => handle.bind_double(i, *v),
            Null => handle.bind_null(i),
        };

        if status != SQLITE_OK {
            return Err(handle.last_error().into());
        }

        Ok(())
    }
}

pub trait IntoArguments: Sized + Send {
    fn into_arguments(self) -> Arguments;
}
