use musq::Decode;

#[derive(Decode)]
#[musq(try_from = "String")]
struct Id(String);

impl TryFrom<String> for Id {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        (!value.is_empty()).then_some(Self(value)).ok_or("empty id")
    }
}

#[derive(Decode)]
#[musq(try_from = "String")]
struct GenericId<T>(T);

impl<T> TryFrom<String> for GenericId<T>
where
    T: From<String>,
{
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        (!value.is_empty())
            .then(|| Self(T::from(value)))
            .ok_or("empty generic id")
    }
}

fn main() {
    let _: Option<GenericId<String>> = None;
}
