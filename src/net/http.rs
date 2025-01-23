use std::{collections::HashMap, net::SocketAddr, sync::Arc};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;

use super::{buffer::DynamicBuffer, ssl_tls::configure_tls};

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub socket: SocketAddr,
}

impl HttpRequest {
    pub fn parser(raw_request: &[u8], socket: SocketAddr) -> Result<Self, String> {
        let request_str = String::from_utf8_lossy(raw_request);
        let mut lines = request_str.lines();

        let first_line = lines.next().ok_or("No first line")?;
        let mut parts = first_line.split_whitespace();
        let method = parts.next().ok_or("No method")?.to_string();
        let path = parts.next().ok_or("No path")?.to_string();

        let mut headers = HashMap::new();
        for line in lines.by_ref() {
            if line.is_empty() {
                break;
            }

            if let Some((key, value)) = line.split_once(':') {
                headers.insert(key.trim().to_string(), value.trim().to_string());
            }
        }

        let body = if let Some(content_length) = headers.get("Content-Length") {
            let content_length = content_length.parse::<usize>().map_err(|e| e.to_string())?;
            let body_start = request_str.find("\r\n\r\n").unwrap_or(0) + 4;
            raw_request[body_start..body_start + content_length].to_vec()
        } else {
            Vec::new()
        };

        Ok(HttpRequest {
            method,
            body,
            headers,
            path,
            socket,
        })
    }
}

pub struct HttpResponse {
    pub status_code: u16,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
}

pub type Handler = Arc<dyn Fn(&mut Writer, HttpRequest) + Send + Sync>;

pub struct Router {
    routes: HashMap<String, Handler>,
}

impl Router {
    pub fn new() -> Self {
        Router {
            routes: HashMap::new(),
        }
    }

    /// Registers a new route with the given path and handler function.
    ///
    /// # Examples
    ///
    /// ```
    /// let mut router = Router::new();
    /// router.handle_func("/example", Arc::new(|w: &mut Writer, r: HttpRequest| {
    ///     w.write(b"Hello, world!");
    /// }));
    /// ```
    /// ### Using middlewares
    /// 
    /// ```rust
    /// fn middleware(handler: Handler) -> Handler {
    ///     Arc::new(move |w: &mut Writer, r: HttpRequest| {
    ///         w.write(b"This a middleware")
    ///         handler(w, r);
    ///     })
    /// }
    /// 
    /// router.handle_func("/example", middleware(handler));
    /// ```
    /// Creates a deep copy of `self`. This is useful when you need to reuse a router in multiple places,
    /// but each place needs to have its own distinct set of handlers.
    pub fn handle_func(&mut self, path: &str, handler: Handler) {
        self.routes.insert(path.to_string(), handler);
    }

    pub fn get_handler(&self, path: &str) -> Option<&Handler> {
        self.routes.get(path)
    }
}

impl Clone for Router {
    fn clone(&self) -> Self {
        Router {
            routes: self.routes.clone(),
        }
    }
}

pub struct Header {
    headers: HashMap<String, String>,
}

impl Header {
    fn new() -> Self {
        Header {
            headers: HashMap::new(),
        }
    }

    pub fn get(&self, key: &str) {
        self.headers.get(key);
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.add(key, value);
    }

    pub fn del(&mut self, key: &str) {
        self.headers.remove(key);
    }

    fn add(&mut self, key: &str, value: &str) {
        self.headers.insert(key.to_string(), value.to_string());
    }
}

pub struct Writer {
    header: Header,
    status_code: u16,
    body: Vec<u8>,
}

impl Writer {
    pub fn header(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn write(&mut self, data: &[u8]) {
        self.body = data.to_vec();
    }

    pub fn write_header(&mut self, status_code: u16) {
        self.status_code = status_code;
    }
}

pub async fn init_tls(router: Router, addrs: &str, cert_path: &str, key_path: &str) {
    let listener = tokio::net::TcpListener::bind(addrs)
        .await
        .expect("Failed to bind address");

    let tls_acceptor = Some(TlsAcceptor::from(configure_tls(cert_path, key_path)));

    loop {
        match listener.accept().await {
            Ok((stream, socket)) => {
                let tls_acceptor = tls_acceptor.clone();
                let router_clone = router.clone();

                tokio::spawn(async move {
                    if let Some(acceptor) = tls_acceptor {
                        match acceptor.accept(stream).await {
                            Ok(stream) => {
                                handle_connection(stream, socket, &router_clone).await;
                            }
                            Err(e) => {
                                eprintln!("Failed to accept TLS connection: {}", e);
                            }
                        }
                    } else {
                        handle_connection(stream, socket, &router_clone).await;
                    }
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}

pub async fn init(router: Router, addrs: &str) {
    let listener = tokio::net::TcpListener::bind(addrs)
        .await
        .expect("Failed to bind address");

    loop {
        match listener.accept().await {
            Ok((stream, socket)) => {
                let router_clone = router.clone();

                tokio::spawn(async move {
                    handle_connection(stream, socket, &router_clone).await;
                });
            }
            Err(e) => {
                eprintln!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection<T>(mut stream: T, socket: SocketAddr, router: &Router)
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buffer = DynamicBuffer::new();
    if let Err(e) = buffer.read_from_stream(&mut stream).await {
        println!("Failed to read from stream: {}", e);
        return;
    }

    let req = HttpRequest::parser(buffer.as_bytes(), socket);
    if let Err(e) = &req {
        println!("Failed to parse request: {}", e);
        return;
    }

    let req = req.unwrap();

    let mut writer = Writer {
        header: Header::new(),
        body: Vec::new(),
        status_code: 200,
    };

    if let Some(handler) = router.get_handler(&req.path) {
        handler(&mut writer, req);
        let response = HttpResponse {
            status_code: writer.status_code,
            headers: writer.header().headers.clone(),
            body: writer.body,
        };

        send_response(&mut stream, response).await;
    } else {
        let response = HttpResponse {
            status_code: 404,
            headers: HashMap::new(),
            body: b"Not Found".to_vec(),
        };
        send_response(&mut stream, response).await;
    }
}

async fn send_response<T>(mut stream: T, response: HttpResponse)
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let status_line = format!("HTTP/1.1 {} OK\r\n", response.status_code);
    let headers = response
        .headers
        .iter()
        .map(|(k, v)| format!("{}: {}\r\n", k, v))
        .collect::<String>();
    let response = format!(
        "{}{}\r\n{}",
        status_line,
        headers,
        String::from_utf8_lossy(&response.body)
    );

    if let Err(e) = stream.write(response.as_bytes()).await {
        println!("Failed to send response: {}", e);
        return;
    }

    stream.flush().await.unwrap();
}
