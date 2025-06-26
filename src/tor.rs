use std::path::Path;

use tokio::{
    fs,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpStream,
};
use tracing::{error, info, warn};

use crate::error::TrackerError;

pub(crate) async fn check_tor_status(
    control_port: u16,
    password: &str,
) -> Result<(), TrackerError> {
    let (reader, mut writer) = TcpStream::connect(format!("127.0.0.1:{control_port}"))
        .await?
        .into_split();
    let mut reader = BufReader::new(reader);
    let auth_command = format!("AUTHENTICATE \"{password}\"\r\n");
    writer.write_all(auth_command.as_bytes()).await?;
    let mut response = String::new();
    reader.read_line(&mut response).await?;
    if !response.starts_with("250") {
        error!(
            "Tor authentication failed: {}, please provide correct password",
            response
        );
        return Err(TrackerError::General(
            "Tor authentication failed".to_string(),
        ));
    }
    writer
        .write_all(b"GETINFO status/bootstrap-phase\r\n")
        .await?;
    response.clear();
    reader.read_line(&mut response).await?;

    if response.contains("PROGRESS=100") {
        info!("Tor is fully started and operational!");
    } else {
        warn!("Tor is still starting, try again later: {}", response);
    }
    Ok(())
}

pub(crate) async fn get_emphemeral_address(
    control_port: u16,
    target_port: u16,
    password: &str,
    private_key_data: Option<&str>,
    service_id_data: Option<&str>,
) -> Result<(String, String), TrackerError> {
    let (reader, mut writer) = TcpStream::connect(format!("127.0.0.1:{control_port}"))
        .await?
        .into_split();
    let mut reader = BufReader::new(reader);
    let mut response = String::new();
    let mut service_id = String::new();
    let mut private_key = String::new();
    let auth_command = format!("AUTHENTICATE \"{password}\"\r\n");
    writer.write_all(auth_command.as_bytes()).await?;
    if let Some(service_id) = service_id_data {
        let remove_command = format!("DEL_ONION {service_id}\r\n");
        writer.write_all(remove_command.as_bytes()).await?;
    }
    let mut add_onion_command =
        format!("ADD_ONION NEW:BEST Flags=Detach Port={target_port},127.0.0.1:{target_port}\r\n");
    if let Some(pk) = private_key_data {
        add_onion_command =
            format!("ADD_ONION {pk} Flags=Detach Port={target_port},127.0.0.1:{target_port}\r\n");
        private_key = pk.to_string();
    }
    writer.write_all(add_onion_command.as_bytes()).await?;

    while reader.read_line(&mut response).await? > 0 {
        if response.starts_with("250-ServiceID=") {
            service_id = response
                .trim_start_matches("250-ServiceID=")
                .trim()
                .to_string();
            if private_key_data.is_some() {
                break;
            }
        } else if response.starts_with("250-PrivateKey=") {
            private_key = response
                .trim_start_matches("250-PrivateKey=")
                .trim()
                .to_string();
            break;
        }
        response.clear();
    }

    if service_id.is_empty() || private_key.is_empty() {
        return Err(TrackerError::General(
            "Failed to retrieve ephemeral onion service details".to_string(),
        ));
    }
    Ok((format!("{service_id}.onion"), private_key))
}

pub(crate) async fn get_tor_hostname(
    data_dir: &Path,
    control_port: u16,
    target_port: u16,
    password: &str,
) -> Result<String, TrackerError> {
    let tor_config_path = data_dir.join("tor/hostname");

    if tor_config_path.exists()
        && let Ok(tor_metadata) = fs::read(&tor_config_path).await
    {
        let data: [&str; 2] = serde_cbor::de::from_slice(&tor_metadata)?;

        let hostname_data = data[1];
        let private_key_data = data[0];

        let (hostname, private_key) = get_emphemeral_address(
            control_port,
            target_port,
            password,
            Some(private_key_data),
            Some(hostname_data.replace(".onion", "").as_str()),
        )
        .await?;

        assert_eq!(hostname, hostname_data);
        assert_eq!(private_key, private_key_data);

        info!(
            "Generated existing Tor Hidden Service Hostname: {}",
            hostname
        );

        return Ok(hostname);
    }

    let (hostname, private_key) =
        get_emphemeral_address(control_port, target_port, password, None, None).await?;

    if let Some(parent) = tor_config_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    fs::write(
        &tor_config_path,
        serde_cbor::ser::to_vec(&[private_key, hostname.clone()])?,
    )
    .await?;

    info!("Generated new Tor Hidden Service Hostname: {}", hostname);

    Ok(hostname)
}
