//! Regression tests for NULL decoding behavior.

mod support;

#[cfg(test)]
mod tests {
    use musq::{DecodeError, Error, query_scalar};

    use crate::support::connection;

    fn assert_unexpected_null(err: Error) {
        match err {
            Error::ColumnDecode {
                source: DecodeError::Conversion(msg),
                ..
            } => assert_eq!(msg, "unexpected NULL"),
            _ => panic!("expected column decode conversion error, got {err:?}"),
        }
    }

    #[tokio::test]
    async fn null_decoding_errors_for_non_option_types() -> anyhow::Result<()> {
        let conn = connection().await?;

        assert_unexpected_null(
            query_scalar::<i32>("SELECT NULL")
                .fetch_one(&conn)
                .await
                .unwrap_err(),
        );
        assert_unexpected_null(
            query_scalar::<i64>("SELECT NULL")
                .fetch_one(&conn)
                .await
                .unwrap_err(),
        );
        assert_unexpected_null(
            query_scalar::<f64>("SELECT NULL")
                .fetch_one(&conn)
                .await
                .unwrap_err(),
        );
        assert_unexpected_null(
            query_scalar::<String>("SELECT NULL")
                .fetch_one(&conn)
                .await
                .unwrap_err(),
        );
        assert_unexpected_null(
            query_scalar::<Vec<u8>>("SELECT NULL")
                .fetch_one(&conn)
                .await
                .unwrap_err(),
        );

        Ok(())
    }

    #[tokio::test]
    async fn null_decoding_is_none_for_option_types() -> anyhow::Result<()> {
        let conn = connection().await?;

        let v: Option<i32> = query_scalar("SELECT NULL").fetch_one(&conn).await?;
        assert_eq!(v, None);

        let v: Option<String> = query_scalar("SELECT NULL").fetch_one(&conn).await?;
        assert_eq!(v, None);

        Ok(())
    }
}
