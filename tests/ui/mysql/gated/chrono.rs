fn main() {
    let _ = query!("select CONVERT(now(), DATE) date");

    let _ = query!("select CONVERT(now(), TIME) time");

    let _ = query!("select CONVERT(now(), DATETIME) datetime");
}
