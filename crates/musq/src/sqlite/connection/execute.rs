use either::Either;

use crate::{
    QueryResult, Result, Row,
    logger::QueryLogger,
    sqlite::{
        Arguments,
        connection::{ConnectionHandle, ConnectionState},
        statement::{CompoundStatement, StatementHandle},
    },
};

/// Iterator over results from executing a compound SQLite statement.
pub struct ExecuteIter<'a> {
    /// Active SQLite connection handle.
    handle: &'a mut ConnectionHandle,
    /// Prepared compound statement.
    statement: &'a mut CompoundStatement,
    /// Query logger when statement or slow-statement logging is enabled.
    logger: Option<QueryLogger<'a>>,
    /// Bound arguments, if any.
    args: Option<Arguments>,

    /// Number of arguments bound so far across statements in the compound
    /// query.
    args_used: usize,

    /// Whether to advance to the next statement on the next iteration.
    goto_next: bool,
}

/// Create an iterator for executing a SQL query with optional arguments.
pub fn iter<'a>(
    conn: &'a mut ConnectionState,
    query: &'a str,
    args: Option<Arguments>,
) -> Result<ExecuteIter<'a>> {
    // fetch the cached statement or allocate a new one
    let statement = conn.statements.get(query)?;

    let logger = conn
        .log_settings
        .is_enabled()
        .then(|| QueryLogger::new(query, conn.log_settings));

    Ok(ExecuteIter {
        handle: &mut conn.handle,
        statement,
        logger,
        args,
        args_used: 0,
        goto_next: true,
    })
}

/// Bind arguments to the provided statement handle.
fn bind(
    statement: &StatementHandle,
    arguments: &Option<Arguments>,
    offset: usize,
) -> Result<usize> {
    let mut n = 0;

    if let Some(arguments) = arguments {
        n = arguments.bind(statement, offset)?;
    }

    Ok(n)
}

impl Iterator for ExecuteIter<'_> {
    type Item = Result<Either<QueryResult, Row>>;

    fn next(&mut self) -> Option<Self::Item> {
        let statement = if self.goto_next {
            let statement = match self.statement.prepare_next(self.handle) {
                Ok(Some(statement)) => statement,
                Ok(None) => return None,
                Err(e) => return Some(Err(e)),
            };

            self.goto_next = false;

            // sanity check: ensure the VM is reset and the bindings are cleared
            if let Err(e) = statement.handle.reset() {
                return Some(Err(e.into()));
            }

            statement.handle.clear_bindings();

            match bind(statement.handle, &self.args, self.args_used) {
                Ok(args_used) => self.args_used += args_used,
                Err(e) => return Some(Err(e)),
            }

            statement
        } else {
            self.statement.current()?
        };

        match statement.handle.step() {
            Ok(true) => {
                if let Some(logger) = &mut self.logger {
                    logger.inc_rows_returned();
                }

                match Row::current(statement.handle, statement.columns, statement.column_names) {
                    Ok(row) => Some(Ok(Either::Right(row))),
                    Err(e) => Some(Err(e)),
                }
            }
            Ok(false) => {
                let last_insert_rowid = self.handle.last_insert_rowid();

                let changes = if statement.handle.is_readonly() {
                    0
                } else {
                    statement.handle.changes()
                };
                if let Some(logger) = &mut self.logger {
                    logger.inc_rows_affected(changes);
                }

                let done = QueryResult {
                    changes,
                    last_insert_rowid,
                };

                self.goto_next = true;

                Some(Ok(Either::Left(done)))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

impl Drop for ExecuteIter<'_> {
    fn drop(&mut self) {
        self.statement.reset().ok();
    }
}
