use crate::Row;
use crate::error::Result;

/// A record that can be built from a row returned by the database.
///
/// To use [`query_as`](crate::query_as) the output type must implement `FromRow`.
///
/// ## Derivable
///
/// This trait can be derived for structs. The generated implementation will make a sequence of calls to
/// [`Row::get_value`] using the name from each struct field.
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct User {
///     id: i32,
///     name: String,
/// }
/// ```
///
/// ### Field attributes
///
/// Several attributes can be specified to customize how each column in a row is read:
///
/// #### `rename`
///
/// When the name of a field in Rust does not match the name of its corresponding column, you can use the `rename`
/// attribute to specify the name that the field has in the row. For example:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct User {
///     id: i32,
///     name: String,
///     #[musq(rename = "description")]
///     about_me: String
/// }
/// ```
///
/// Given a query such as:
///
/// ```sql
/// SELECT id, name, description FROM users;
/// ```
///
/// will read the content of the column `description` into the field `about_me`.
///
/// #### `rename_all`
/// By default, field names are expected verbatim (with the exception of the raw identifier prefix `r#`, if present).
/// Placed at the struct level, this attribute changes how the field name is mapped to its SQL column name:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// #[musq(rename_all = "camelCase")]
/// struct UserPost {
///     id: i32,
///     // remapped to "userId"
///     user_id: i32,
///     contents: String
/// }
/// ```
///
/// The supported values are `snake_case` (available if you have non-snake-case field names for some reason),
/// `lowercase`, `UPPERCASE`, `camelCase`, `PascalCase`, `SCREAMING_SNAKE_CASE` and `kebab-case`. The styling of each
/// option is intended to be an example of its behavior.
///
/// #### `default`
///
/// When your struct contains a field that is not present in your query, if the field type has an implementation for
/// [`Default`], you can use the `default` attribute to assign the default value to said field. For example:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct User {
///     id: i32,
///     name: String,
///     #[musq(default)]
///     location: Option<String>
/// }
/// ```
///
/// Given a query such as:
///
/// ```sql
/// SELECT id, name FROM users;
/// ```
///
/// will set the value of the field `location` to the default value of `Option<String>`, which is `None`.
///
/// ### `flatten`
///
/// If you want to handle a field that implements [`FromRow`], you can use the `flatten` attribute to specify that you
/// want it to use [`FromRow`] for parsing rather than the usual method. For example:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct Address {
///     country: String,
///     city: String,
///     road: String,
/// }
///
/// #[derive(FromRow)]
/// struct User {
///     id: i32,
///     name: String,
///     #[musq(flatten)]
///     address: Address,
/// }
/// ```
/// Given a query such as:
///
/// ```sql
/// SELECT id, name, country, city, road FROM users;
/// ```
///
/// This field is compatible with the `default` attribute.
///
/// #### `skip`
///
/// This is a variant of the `default` attribute which instead always takes the value from the `Default` implementation
/// for this field type ignoring any results in your query. This can be useful, if some field does not satifisfy the
/// trait bounds (i.e. `decode::Decode`, `type::Type`), in particular in case of nested structures. For example:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct Address {
///     user_name: String,
///     street: String,
///     city: String,
/// }
///
/// #[derive(FromRow)]
/// struct User {
///     name: String,
///     #[musq(skip)]
///     addresses: Vec<Address>,
/// }
/// ```
///
/// Then when querying into `User`, only `name` needs to be set:
///
/// ```rust,ignore
/// let user: User = query_as("SELECT name FROM users")
///    .fetch_one(&mut some_connection)
///    .await?;
///
/// `Default` for `Vec<Address>` is an empty vector.
/// assert!(user.addresses.is_empty());
/// ```
///
/// ## Manual implementation
///
/// You can also implement the [`FromRow`] trait by hand. This can be useful if you have a struct with a field that
/// needs manual decoding:
///
///
/// ```rust,ignore
/// use {FromRow, Row};
/// struct MyCustomType {
///     custom: String,
/// }
///
/// struct Foo {
///     bar: MyCustomType,
/// }
///
/// impl FromRow<'_> for Foo {
///     fn from_row(row: &Row) -> Result<Self> {
///         Ok(Self {
///             bar: MyCustomType {
///                 custom: row.get_value("custom")?
///             }
///         })
///     }
/// }
/// ```
///
/// #### `try_from`
///
/// When your struct contains a field whose type is not matched with the database type, if the field type has an
/// implementation [`TryFrom`] for the database type, you can use the `try_from` attribute to convert the database type
/// to the field type. For example:
///
/// ```rust,ignore
/// #[derive(FromRow)]
/// struct User {
///     id: i32,
///     name: String,
///     #[musq(try_from = "i64")]
///     score: u64
/// }
/// ```
pub trait FromRow<'r>: Sized {
    fn from_row(prefix: &str, row: &'r Row) -> Result<Self>;
}

/// Helper trait used internally to determine if all columns belonging to a
/// record are `NULL` in a given row.
pub trait AllNull<'r> {
    fn all_null(prefix: &str, row: &'r Row) -> Result<bool>;
}

impl<'r, T> AllNull<'r> for Option<T>
where
    T: AllNull<'r>,
{
    fn all_null(prefix: &str, row: &'r Row) -> Result<bool> {
        T::all_null(prefix, row)
    }
}

/// Implementation of [`FromRow`] for optional nested records. The value is set
/// to `None` if [`AllNull::all_null`] returns `true` for the inner type.
impl<'r, T> FromRow<'r> for Option<T>
where
    T: FromRow<'r> + AllNull<'r>,
{
    fn from_row(prefix: &str, row: &'r Row) -> Result<Self> {
        if <T as AllNull>::all_null(prefix, row)? {
            Ok(None)
        } else {
            Ok(Some(T::from_row(prefix, row)?))
        }
    }
}

// implement FromRow for tuples of types that implement Decode
// up to tuples of 9 values

macro_rules! impl_from_row_for_tuple {
    ($( ($idx:tt) -> $T:ident );+;) => {
        impl<'r, $($T,)+> FromRow<'r> for ($($T,)+)
        where
            $($T: crate::decode::Decode<'r>,)+
        {

            fn from_row(_prefix: &str, row: &'r Row) -> Result<Self> {
                Ok(($(row.get_value_idx($idx as usize)?,)+))
            }
        }
    };
}

impl_from_row_for_tuple!(
    (0) -> T1;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
    (11) -> T12;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
    (11) -> T12;
    (12) -> T13;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
    (11) -> T12;
    (12) -> T13;
    (13) -> T14;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
    (11) -> T12;
    (12) -> T13;
    (13) -> T14;
    (14) -> T15;
);

impl_from_row_for_tuple!(
    (0) -> T1;
    (1) -> T2;
    (2) -> T3;
    (3) -> T4;
    (4) -> T5;
    (5) -> T6;
    (6) -> T7;
    (7) -> T8;
    (8) -> T9;
    (9) -> T10;
    (10) -> T11;
    (11) -> T12;
    (12) -> T13;
    (13) -> T14;
    (14) -> T15;
    (15) -> T16;
);
