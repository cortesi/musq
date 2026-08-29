#[cfg(feature = "json")]
use musq::Json;
#[cfg(feature = "json")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "json")]
#[derive(Json, Serialize, Deserialize)]
struct Generic {
    val: String,
}

#[cfg(feature = "json")]
#[derive(Json, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Tagged {
    Unit,
    Tuple(String),
    Named { value: String },
}

fn main() {}
