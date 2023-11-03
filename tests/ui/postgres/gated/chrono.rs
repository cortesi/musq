fn main() {
    let _ = query!("select now()::date");

    let _ = query!("select now()::time");

    let _ = query!("select now()::timestamp");

    let _ = query!("select now()::timestamptz");

    let _ = query!("select $1::date", ());

    let _ = query!("select $1::time", ());

    let _ = query!("select $1::timestamp", ());

    let _ = query!("select $1::timestamptz", ());
}
