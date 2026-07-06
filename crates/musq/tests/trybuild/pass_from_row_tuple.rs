#![deny(warnings)]

use musq::FromRow;

#[allow(dead_code)]
#[derive(FromRow)]
struct Pair(i32, String);

#[allow(dead_code)]
#[derive(FromRow)]
struct Generic<T>(T, i64);

#[allow(dead_code)]
#[derive(FromRow)]
struct Borrowed<'r, T>(&'r T, i32);

fn main() {}
