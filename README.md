# Routs

**Rust Reference** | **CI** | **Rust API**

![Routs Logo]("")

**Routs** is a Rust library that provides a lightweight and asynchronous HTTP server implementation. It is designed to handle routing, request parsing, and response generation with ease. Routs supports TLS/SSL for secure connections and allows you to define custom handlers and middlewares for your routes.

If you would like to contribute to Routs or report issues, please visit the [GitHub repository](https://github.com/Murilinho145SG/Ro).

For help with this library or general Rust discussion, feel free to join the Rust community channels.

---

## Getting Started

### Installing

This assumes you already have a working Rust environment. If not, please see the [official Rust installation guide](https://www.rust-lang.org/tools/install) first.

Add the following to your `Cargo.toml` file:

toml

Copy

```
[dependencies]
routs = { git = "https://github.com/your-repo/routs.git" }
```

Then, run `cargo build` to fetch and compile the library.

---

## Usage

### Basic Example

Here’s a simple example of how to use Routs to create an HTTP server with a single route:

rust

Copy

```
use routs::{Router, HttpRequest, Writer};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let mut router = Router::new();

    // Register a handler for the root path ("/")
    router.handle_func("/", Arc::new(|w: &mut Writer, _: HttpRequest| {
        w.write_header(200);
        w.write(b"Hello, world!");
    }));

    // Start the server on localhost:8080
    routs::init(router, "127.0.0.1:8080").await;
}
```

### Using Middlewares

You can easily add middlewares to your handlers. Here’s an example:

rust

Copy

```
use routs::{Router, HttpRequest, Writer, Handler};
use std::sync::Arc;

fn logging_middleware(handler: Handler) -> Handler {
    Arc::new(move |w: &mut Writer, r: HttpRequest| {
        println!("Request received: {} {}", r.method, r.path);
        handler(w, r); // Call the next handler
    })
}

#[tokio::main]
async fn main() {
    let mut router = Router::new();

    let handler = Arc::new(|w: &mut Writer, _: HttpRequest| {
        w.write_header(200);
        w.write(b"Hello from the handler!");
    });

    // Apply the middleware to the handler
    router.handle_func("/middleware", logging_middleware(handler));

    routs::init(router, "127.0.0.1:8080").await;
}
```

### Enabling TLS/SSL

To enable secure connections, use the `init_tls` function and provide paths to your certificate and key files:

rust

Copy

```
#[tokio::main]
async fn main() {
    let mut router = Router::new();

    router.handle_func("/", Arc::new(|w: &mut Writer, _: HttpRequest| {
        w.write_header(200);
        w.write(b"Secure Hello, world!");
    }));

    // Start the server with TLS on localhost:8443
    routs::init_tls(
        router,
        "127.0.0.1:8443",
        "path/to/cert.pem",
        "path/to/key.pem"
    ).await;
}
```

---

## Features

* **Asynchronous HTTP Server**: Built on top of `tokio` for high-performance asynchronous I/O.
* **Request Parsing**: Automatically parses HTTP requests into a structured format.
* **Routing**: Easily define routes and handlers for different paths.
* **Middlewares**: Add reusable middlewares to your handlers.
* **TLS/SSL Support**: Secure your server with TLS/SSL encryption.

---

## Contributing

Contributions are welcome! If you’d like to contribute to Routs, please:

1. Fork the repository.
2. Create a new branch for your feature or bugfix.
3. Submit a pull request.

---

## License

Routs is licensed under the MIT License. See the [LICENSE](https://chat.deepseek.com/a/chat/s/5ac7a2d0-551a-4898-8334-9fb537d2a59f#) file for details.

---

Feel free to customize this further to match your project’s branding or specific details!
