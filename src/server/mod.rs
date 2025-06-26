mod tracker_monitor;
mod tracker_server;
use tokio::{
    io::{AsyncWriteExt, BufWriter},
    net::tcp::WriteHalf,
};

pub use tracker_server::run;

use crate::error::TrackerError;

/// This method adds a prefix for
/// maker to identify if its
/// taker or not
pub async fn send_message_with_prefix(
    writer: &mut BufWriter<WriteHalf<'_>>,
    message: &impl serde::Serialize,
) -> Result<(), TrackerError> {
    let mut msg_bytes = Vec::new();
    msg_bytes.push(0x02);
    msg_bytes.extend(serde_cbor::to_vec(message)?);
    let msg_len = (msg_bytes.len() as u32).to_be_bytes();
    let mut to_send = Vec::with_capacity(msg_bytes.len() + msg_len.len());
    to_send.extend(msg_len);
    to_send.extend(msg_bytes);
    writer.write_all(&to_send).await?;
    writer.flush().await?;
    Ok(())
}
