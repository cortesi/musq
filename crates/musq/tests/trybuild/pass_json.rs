use musq::Json;
use serde::{Deserialize, Serialize};

#[derive(Json, Serialize, Deserialize)]
struct Generic {
    val: String,
}

#[derive(Json, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Tagged {
    Unit,
    Tuple(String),
    Named { value: String },
}

fn main() {}
