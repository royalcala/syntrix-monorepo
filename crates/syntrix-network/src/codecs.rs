use std::io;

use async_trait::async_trait;
use futures::AsyncReadExt;
use libp2p::request_response::Codec;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct InvitePayload {
    pub org_name: String,
    pub role: String,
    pub admin_addr: Option<String>,
    pub topic_id: String,
    pub can_open: Vec<String>,
    pub can_write: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkRequest {
    Invite(InvitePayload),
    CatchupRequest { org_id: String, since_hlc: u64 },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum NetworkResponse {
    InviteAck,
    CatchupResponse(Vec<serde_json::Value>),
}

/// Combined codec that handles both invite and catchup protocols.
#[derive(Debug, Clone, Default)]
pub struct CombinedCodec;

#[async_trait]
impl Codec for CombinedCodec {
    type Protocol = String;
    type Request = NetworkRequest;
    type Response = NetworkResponse;

    async fn read_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Request>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn read_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T) -> io::Result<Self::Response>
    where
        T: futures::AsyncRead + Unpin + Send,
    {
        let mut buf = Vec::new();
        io.read_to_end(&mut buf).await?;
        serde_json::from_slice(&buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    async fn write_request<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, req: Self::Request) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&req).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        futures::AsyncWriteExt::write_all(io, &data).await?;
        futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }

    async fn write_response<T>(&mut self, _protocol: &Self::Protocol, io: &mut T, res: Self::Response) -> io::Result<()>
    where
        T: futures::AsyncWrite + Unpin + Send,
    {
        let data = serde_json::to_vec(&res).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        futures::AsyncWriteExt::write_all(io, &data).await?;
        futures::AsyncWriteExt::close(io).await?;
        Ok(())
    }
}
