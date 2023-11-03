fn main() {
    let _query = query!("select $1::text", 0i32);

    let _query = query!("select $1::text", &0i32);

    let _query = query!("select $1::text", Some(0i32));

    let arg = 0i32;
    let _query = query!("select $1::text", arg);

    let arg = Some(0i32);
    let _query = query!("select $1::text", arg);
    let _query = query!("select $1::text", arg.as_ref());
}
