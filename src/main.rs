use std::sync::Arc;

use routs::http::{self, HttpRequest, HttpStatus, Writer};

mod routs;

#[tokio::main]
async fn main() {
    let mut router = http::Router::new();

    router.handle_func("/", Arc::new(|w: &mut Writer, r: HttpRequest| {
            w.header().set("Access-Control-Allow-Methods", "GET");

            if r.method != "POST" {
                w.write_header(HttpStatus::MethodNotAllowed);
                return;
            }

            println!("{}", String::from_utf8_lossy(&r.body));

            w.write_header(HttpStatus::OK);
        }),
    );

    http::init(router, "0.0.0.0:8080").await;
}