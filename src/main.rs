use std::{fs::File, io::BufReader, sync::Arc};

use net::http::{self, Handler, HttpRequest, Writer};
use serde::{Deserialize, Serialize};
use serde_json::json;

mod net;

fn middleware(handler: Handler) -> Handler {
    Arc::new(move |w: &mut Writer, r: HttpRequest| {
        println!("{:?}", r);
        handler(w, r);
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct Infos {
    cert_file: String,
    key_file: String
}

#[tokio::main]
async fn main() {
    let file = File::open("./info.json").expect("Erro ao ler o json");
    let reader = BufReader::new(file);
    let json: Infos = serde_json::from_reader(reader).expect("Erro ao converter o json para struct");

    println!("{:?}", json);
    
    let mut router = http::Router::new();

    router.handle_func("/", middleware(Arc::new(|w: &mut Writer, r: HttpRequest| {
        w.header().set("Content-Type", "application/json");
        w.header().set("Access-Control-Allow-Methods", "POST, OPTIONS");

        if r.method == "OPTIONS" {
            w.write_header(200);
            return;
        }

        if r.method != "POST" {
            w.write_header(405);
            return;
        }
        
        println!("{:?}", String::from_utf8_lossy(&r.body));
        w.write(json!({"message": "alelo"}).to_string().as_bytes());
    })));

    http::init_tls(router, "0.0.0.0:7643", &json.cert_file, &json.key_file).await;
}