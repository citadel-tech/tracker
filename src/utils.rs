use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter},
    net::tcp::{ReadHalf, WriteHalf},
};

use crate::error::TrackerError;

pub async fn read_message(reader: &mut BufReader<ReadHalf<'_>>) -> Result<Vec<u8>, TrackerError> {
    // length of incoming data
    let mut len_buff = [0u8; 4];
    reader.read_exact(&mut len_buff).await?;
    let length = u32::from_be_bytes(len_buff);
    let mut buffer = vec![0; length as usize];

    _ = reader.read(&mut buffer[4..]).await?;

    Ok(buffer)
}

pub async fn send_message(
    writer: &mut BufWriter<WriteHalf<'_>>,
    message: &impl serde::Serialize,
) -> Result<(), TrackerError> {
    let msg_bytes = serde_cbor::ser::to_vec(message)?;
    let msg_len = (msg_bytes.len() as u32).to_be_bytes();
    let mut to_send = Vec::with_capacity(msg_bytes.len() + msg_len.len());
    to_send.extend(msg_len);
    to_send.extend(msg_bytes);
    writer.write_all(&to_send).await?;
    writer.flush().await?;
    Ok(())
}
