use std::sync::Arc;

use routs::http::{self, HttpRequest, Writer};
use serde_json::json;

mod routs;

#[tokio::main]
async fn main() {
    let mut router = http::Router::new();

    router.handle_func("/", Arc::new(|w: &mut Writer, r: HttpRequest| {
            w.header().set("Access-Control-Allow-Methods", "GET");

            if r.method != "GET" {
                w.write_header(405);
                return;
            }

            w.write(json!({"message": "Hello World!"}).to_string().as_bytes());
        }),
    );

    http::init(router, "0.0.0.0:7643").await;
}