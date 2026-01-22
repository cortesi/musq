use musq::Json;
use serde::{Deserialize, Serialize};

#[derive(Json, Serialize, Deserialize)]
struct Generic {
    val: String,
}

fn main() {}
