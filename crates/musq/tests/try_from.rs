//! Integration tests for musq.

mod support;

#[cfg(test)]
mod tests {
    use musq::{Error, Value, decode::Decode, query_as};
    use musq_macros::*;

    use crate::support::connection;

    #[derive(Debug, FromRow, PartialEq)]
    struct Foo {
        #[musq(try_from = "i64")]
        value: u64,
    }

    #[tokio::test]
    async fn try_from_failure_maps_error() -> anyhow::Result<()> {
        let conn = connection().await?;

        let res: musq::Result<Foo> = query_as::<Foo>("SELECT -1 as value").fetch_one(&conn).await;

        let err = res.expect_err("expected failure");
        if let Error::ColumnDecode {
            column_name, value, ..
        } = err
        {
            assert_eq!(column_name, "value");
            match value {
                Value::Integer { value, .. } => assert_eq!(value, -1),
                other => panic!("unexpected value: {other:?}"),
            }
        } else {
            panic!("unexpected error: {err:?}");
        }

        Ok(())
    }

    #[derive(Debug, Decode, PartialEq)]
    #[musq(try_from = "String")]
    struct Identifier(String);

    impl TryFrom<String> for Identifier {
        type Error = &'static str;

        fn try_from(value: String) -> Result<Self, Self::Error> {
            value
                .starts_with("id-")
                .then_some(Self(value))
                .ok_or("identifier must start with id-")
        }
    }

    #[test]
    fn decode_try_from_checks_the_newtype() {
        let valid = Value::Text {
            value: "id-valid".into(),
            type_info: None,
        };
        assert_eq!(
            Identifier::decode(&valid).unwrap(),
            Identifier("id-valid".into())
        );

        let invalid = Value::Text {
            value: "invalid".into(),
            type_info: None,
        };
        let error = Identifier::decode(&invalid).unwrap_err();
        assert!(error.to_string().contains("identifier must start with id-"));
    }
}
