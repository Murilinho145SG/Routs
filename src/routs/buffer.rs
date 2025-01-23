
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct DynamicBuffer {
    pub data: Vec<u8>
}

impl DynamicBuffer {
    pub fn new() -> Self {
        DynamicBuffer {
            data: Vec::new()
        }
    }

    pub async fn read_from_stream<T>(&mut self, mut stream: T) -> Result<usize, String> where T: AsyncReadExt + AsyncWriteExt + Unpin, {
        let mut chunk = [0; 1024];
        let size = stream.read(&mut chunk).await.map_err(|e| e.to_string())?;
        self.data.extend_from_slice(&chunk[..size]);
        Ok(size)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}