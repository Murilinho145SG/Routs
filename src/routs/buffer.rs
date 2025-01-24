use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct DynamicBuffer<T> {
    pub headers: Vec<u8>,
    pub stream: T,
    pub body: Vec<u8>,
}

impl<T> DynamicBuffer<T> {
    pub fn new(stream: T) -> Self {
        DynamicBuffer {
            headers: Vec::new(),
            stream,
            body: Vec::new(),
        }
    }

    pub async fn read_headers_and_body(&mut self) -> Result<(), String>
    where
        T: AsyncReadExt + AsyncWriteExt + Unpin,
    {
        let mut buffer = [0; 1024];
        let mut total_read = 0;
        let mut header_end = false;
        let mut content_length = 0;

        loop {
            let bytes_read = self.stream.read(&mut buffer).await.map_err(|e| e.to_string())?;
            if bytes_read == 0 {
                break;
            }

            self.headers.extend_from_slice(&buffer[..bytes_read]);
            total_read += bytes_read;

            if !header_end {
                if let Some(pos) = self.headers.windows(4).position(|window| window == b"\r\n\r\n") {
                    header_end = true;

                    let header_str = String::from_utf8_lossy(&self.headers[..pos]);
                    for line in header_str.lines() {
                        if let Some((key, value)) = line.split_once(':') {
                            if key.trim().eq_ignore_ascii_case("Content-Length") {
                                content_length = value.trim().parse().unwrap_or(0);
                            }
                        }
                    }

                    let remaining = &self.headers[(pos + 4)..];
                    self.body.extend_from_slice(remaining);
                }
            } else {
                self.body.extend_from_slice(&buffer[..bytes_read]);
            }

            if self.body.len() >= content_length {
                break;
            }
        }

        Ok(())
    }
}
