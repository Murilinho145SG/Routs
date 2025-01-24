use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_rustls::TlsAcceptor;
use log::{error, info, warn};

use super::{buffer::DynamicBuffer, ssl_tls::configure_tls};

pub struct HttpRequest {
    pub method: String,
    pub path: String,
    pub headers: HashMap<String, String>,
    pub body: Vec<u8>,
    pub socket: SocketAddr,
}

impl HttpRequest {
    pub async fn parser<T>(mut buffer: DynamicBuffer<T>, socket: SocketAddr) -> Result<Self, String>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let headers = buffer.headers.clone();
        let request_str = String::from_utf8_lossy(&headers);
        let mut lines = request_str.lines();

        let first_line = lines.next().ok_or("Invalid HTTP request: Missing request line")?;
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
        
            while buffer.body.len() < content_length {
                let mut chunk = vec![0; 1024];
                let bytes_read = buffer.stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
                if bytes_read == 0 {
                    return Err("Connection closed before reading full body".to_string());
                }
                buffer.body.extend_from_slice(&chunk[..bytes_read]);
            }
        
            buffer.body[..content_length].to_vec()
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
    pub status_code: HttpStatus,
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
    status_code: HttpStatus,
    body: Vec<u8>,
}

impl Writer {
    pub fn header(&mut self) -> &mut Header {
        &mut self.header
    }

    pub fn write(&mut self, data: &[u8]) {
        self.body = data.to_vec();
    }

    pub fn write_header(&mut self, status_code: HttpStatus) {
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
                                info!("TLS connection accepted from {}", socket);
                                handle_connection(stream, socket, &router_clone).await;
                            }
                            Err(e) => {
                                error!("Failed to accept TLS connection from {}: {}", socket, e);
                            }
                        }
                    } else {
                        handle_connection(stream, socket, &router_clone).await;
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
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
                    info!("Connection accepted from {}", socket);
                    handle_connection(stream, socket, &router_clone).await;
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

async fn handle_connection<T>(mut stream: T, socket: SocketAddr, router: &Router)
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let mut buffer = DynamicBuffer::new(&mut stream);
    if let Err(e) = &buffer.read_headers_and_body().await {
        error!("Failed to read from stream: {}", e);
        return;
    }

    let req = HttpRequest::parser(buffer, socket).await;
    if let Err(e) = &req {
        error!("Failed to parse request: {}", e);
        return;
    }

    let req = req.unwrap();

    let mut writer = Writer {
        header: Header::new(),
        body: Vec::new(),
        status_code: HttpStatus::OK,
    };

    if let Some(handler) = router.get_handler(&req.path) {
        handler(&mut writer, req);
    } else {
        warn!("No handler found for path: {}", req.path);
        writer.status_code = HttpStatus::NotFound;
        writer.body = b"Not Found".to_vec();
    }

    let response = HttpResponse {
        headers: writer.header().headers.clone(),
        status_code: writer.status_code,
        body: writer.body,
    };

    send_response(&mut stream, response).await;
}

async fn send_response<T>(mut stream: T, response: HttpResponse)
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    let status_line = format!("HTTP/1.1 {}\r\n", response.status_code.to_string());
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
        error!("Failed to send response: {}", e);
        return;
    }

    if let Err(e) = stream.flush().await {
        error!("Failed to flush stream: {}", e);
    }
}

pub enum HttpStatus {
    // Informational responses
    Continue,
    SwitchingProtocols,
    Processing,
    EarlyHints,

    // Success responses
    OK,
    Created,
    Accepted,
    NonAuthoritativeInfo,
    NoContent,
    ResetContent,
    PartialContent,
    MultiStatus,
    AlreadyReported,
    IMUsed,

    // Redirection messages
    MultipleChoices,
    MovedPermanently,
    Found,
    SeeOther,
    NotModified,
    UseProxy,
    TemporaryRedirect,
    PermanentRedirect,

    // Client error responses
    BadRequest,
    Unauthorized,
    PaymentRequired,
    Forbidden,
    NotFound,
    MethodNotAllowed,
    NotAcceptable,
    ProxyAuthRequired,
    RequestTimeout,
    Conflict,
    Gone,
    LengthRequired,
    PreconditionFailed,
    RequestEntityTooLarge,
    RequestURITooLong,
    UnsupportedMediaType,
    RequestedRangeNotSatisfiable,
    ExpectationFailed,
    Teapot,
    MisdirectedRequest,
    UnprocessableEntity,
    Locked,
    FailedDependency,
    TooEarly,
    UpgradeRequired,
    PreconditionRequired,
    TooManyRequests,
    RequestHeaderFieldsTooLarge,
    UnavailableForLegalReasons,

    // Server error responses
    InternalServerError,
    NotImplemented,
    BadGateway,
    ServiceUnavailable,
    GatewayTimeout,
    HTTPVersionNotSupported,
    VariantAlsoNegotiates,
    InsufficientStorage,
    LoopDetected,
    NotExtended,
    NetworkAuthenticationRequired,
}

impl HttpStatus {
    pub fn to_string(&self) -> &'static str {
        match self {
            // Informational responses
            HttpStatus::Continue => "100 Continue",
            HttpStatus::SwitchingProtocols => "101 Switching Protocols",
            HttpStatus::Processing => "102 Processing",
            HttpStatus::EarlyHints => "103 Early Hints",

            // Success responses
            HttpStatus::OK => "200 OK",
            HttpStatus::Created => "201 Created",
            HttpStatus::Accepted => "202 Accepted",
            HttpStatus::NonAuthoritativeInfo => "203 Non-Authoritative Information",
            HttpStatus::NoContent => "204 No Content",
            HttpStatus::ResetContent => "205 Reset Content",
            HttpStatus::PartialContent => "206 Partial Content",
            HttpStatus::MultiStatus => "207 Multi-Status",
            HttpStatus::AlreadyReported => "208 Already Reported",
            HttpStatus::IMUsed => "226 IM Used",

            // Redirection messages
            HttpStatus::MultipleChoices => "300 Multiple Choices",
            HttpStatus::MovedPermanently => "301 Moved Permanently",
            HttpStatus::Found => "302 Found",
            HttpStatus::SeeOther => "303 See Other",
            HttpStatus::NotModified => "304 Not Modified",
            HttpStatus::UseProxy => "305 Use Proxy",
            HttpStatus::TemporaryRedirect => "307 Temporary Redirect",
            HttpStatus::PermanentRedirect => "308 Permanent Redirect",

            // Client error responses
            HttpStatus::BadRequest => "400 Bad Request",
            HttpStatus::Unauthorized => "401 Unauthorized",
            HttpStatus::PaymentRequired => "402 Payment Required",
            HttpStatus::Forbidden => "403 Forbidden",
            HttpStatus::NotFound => "404 Not Found",
            HttpStatus::MethodNotAllowed => "405 Method Not Allowed",
            HttpStatus::NotAcceptable => "406 Not Acceptable",
            HttpStatus::ProxyAuthRequired => "407 Proxy Authentication Required",
            HttpStatus::RequestTimeout => "408 Request Timeout",
            HttpStatus::Conflict => "409 Conflict",
            HttpStatus::Gone => "410 Gone",
            HttpStatus::LengthRequired => "411 Length Required",
            HttpStatus::PreconditionFailed => "412 Precondition Failed",
            HttpStatus::RequestEntityTooLarge => "413 Request Entity Too Large",
            HttpStatus::RequestURITooLong => "414 Request-URI Too Long",
            HttpStatus::UnsupportedMediaType => "415 Unsupported Media Type",
            HttpStatus::RequestedRangeNotSatisfiable => "416 Requested Range Not Satisfiable",
            HttpStatus::ExpectationFailed => "417 Expectation Failed",
            HttpStatus::Teapot => "418 I'm a teapot",
            HttpStatus::MisdirectedRequest => "421 Misdirected Request",
            HttpStatus::UnprocessableEntity => "422 Unprocessable Entity",
            HttpStatus::Locked => "423 Locked",
            HttpStatus::FailedDependency => "424 Failed Dependency",
            HttpStatus::TooEarly => "425 Too Early",
            HttpStatus::UpgradeRequired => "426 Upgrade Required",
            HttpStatus::PreconditionRequired => "428 Precondition Required",
            HttpStatus::TooManyRequests => "429 Too Many Requests",
            HttpStatus::RequestHeaderFieldsTooLarge => "431 Request Header Fields Too Large",
            HttpStatus::UnavailableForLegalReasons => "451 Unavailable For Legal Reasons",

            // Server error responses
            HttpStatus::InternalServerError => "500 Internal Server Error",
            HttpStatus::NotImplemented => "501 Not Implemented",
            HttpStatus::BadGateway => "502 Bad Gateway",
            HttpStatus::ServiceUnavailable => "503 Service Unavailable",
            HttpStatus::GatewayTimeout => "504 Gateway Timeout",
            HttpStatus::HTTPVersionNotSupported => "505 HTTP Version Not Supported",
            HttpStatus::VariantAlsoNegotiates => "506 Variant Also Negotiates",
            HttpStatus::InsufficientStorage => "507 Insufficient Storage",
            HttpStatus::LoopDetected => "508 Loop Detected",
            HttpStatus::NotExtended => "510 Not Extended",
            HttpStatus::NetworkAuthenticationRequired => "511 Network Authentication Required",
        }
    }
}