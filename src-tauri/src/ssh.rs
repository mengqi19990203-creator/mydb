use crate::models::ConnectionConfig;
use russh::client;
use russh::keys::{
    Error as KeyError, PrivateKeyWithHashAlg, PublicKeyOrCertificate, load_secret_key,
};
use russh::{ChannelMsg, Disconnect};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

pub struct SshTunnel {
    pub local_port: u16,
    task: JoinHandle<()>,
}

impl SshTunnel {
    pub async fn open(
        config: &ConnectionConfig,
        db_host: &str,
        db_port: u16,
        password: Option<&str>,
    ) -> Result<Self, String> {
        let listener = TcpListener::bind(("127.0.0.1", 0))
            .await
            .map_err(|error| format!("创建本地 SSH 转发端口失败: {error}"))?;
        let local_port = listener
            .local_addr()
            .map_err(|error| format!("读取本地 SSH 端口失败: {error}"))?
            .port();

        let ssh_host = config.ssh_host.clone();
        let ssh_port = if config.ssh_port == 0 {
            22
        } else {
            config.ssh_port
        };
        let ssh_username = config.ssh_username.clone();
        let ssh_auth = config.ssh_auth.clone();
        let ssh_key_path = config.ssh_key_path.clone();
        let db_host = db_host.to_string();
        let password = password.map(|value| value.to_string());

        // Validate the SSH endpoint and credentials before exposing the local port,
        // so authentication errors surface directly instead of as DB timeouts.
        let probe = connect_ssh(
            &ssh_host,
            ssh_port,
            &ssh_username,
            &ssh_auth,
            &ssh_key_path,
            password.as_deref(),
        )
        .await?;
        let _ = probe
            .disconnect(Disconnect::ByApplication, "", "English")
            .await;

        let task = tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, origin)) => {
                        let ssh_host = ssh_host.clone();
                        let ssh_username = ssh_username.clone();
                        let ssh_auth = ssh_auth.clone();
                        let ssh_key_path = ssh_key_path.clone();
                        let db_host = db_host.clone();
                        let password = password.clone();

                        tokio::spawn(async move {
                            if let Err(error) = proxy_connection(
                                stream,
                                origin,
                                &ssh_host,
                                ssh_port,
                                &ssh_username,
                                &ssh_auth,
                                &ssh_key_path,
                                &db_host,
                                db_port,
                                password.as_deref(),
                            )
                            .await
                            {
                                eprintln!("mydb SSH tunnel: {error}");
                            }
                        });
                    }
                    Err(error) => {
                        eprintln!("mydb SSH listener: {error}");
                        break;
                    }
                }
            }
        });

        Ok(Self { local_port, task })
    }
}

impl Drop for SshTunnel {
    fn drop(&mut self) {
        self.task.abort();
    }
}

struct ClientHandler {
    host: String,
    port: u16,
}

impl client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &PublicKeyOrCertificate,
    ) -> Result<bool, Self::Error> {
        let public_key = server_public_key.public_key();

        match russh::keys::check_known_hosts(&self.host, self.port, &public_key) {
            Ok(true) => Ok(true),
            Ok(false) => {
                // Trust on first use: remember the key so later changes are detected.
                let _ =
                    russh::keys::known_hosts::learn_known_hosts(&self.host, self.port, &public_key);
                Ok(true)
            }
            Err(russh::keys::Error::KeyChanged { .. }) => Ok(false),
            Err(_) => Ok(true),
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn proxy_connection(
    mut stream: TcpStream,
    origin: SocketAddr,
    ssh_host: &str,
    ssh_port: u16,
    ssh_username: &str,
    ssh_auth: &str,
    ssh_key_path: &str,
    db_host: &str,
    db_port: u16,
    password: Option<&str>,
) -> Result<(), String> {
    let session = connect_ssh(
        ssh_host,
        ssh_port,
        ssh_username,
        ssh_auth,
        ssh_key_path,
        password,
    )
    .await?;

    let mut channel = session
        .channel_open_direct_tcpip(
            db_host.to_string(),
            db_port as u32,
            origin.ip().to_string(),
            origin.port() as u32,
        )
        .await
        .map_err(|error| format!("打开 SSH 转发通道失败: {error}"))?;

    let mut buffer = vec![0u8; 65536];
    let mut stream_closed = false;

    loop {
        tokio::select! {
            result = stream.read(&mut buffer), if !stream_closed => {
                match result {
                    Ok(0) => {
                        stream_closed = true;
                        channel.eof().await.map_err(|error| format!("关闭 SSH 写入失败: {error}"))?;
                    }
                    Ok(count) => {
                        channel.data(&buffer[..count]).await.map_err(|error| format!("SSH 写入失败: {error}"))?;
                    }
                    Err(error) => return Err(format!("本地连接读取失败: {error}")),
                }
            }
            Some(message) = channel.wait() => {
                match message {
                    ChannelMsg::Data { ref data } => {
                        stream.write_all(data).await.map_err(|error| format!("本地连接写入失败: {error}"))?;
                    }
                    ChannelMsg::Eof => {
                        if !stream_closed {
                            let _ = channel.eof().await;
                        }
                        break;
                    }
                    ChannelMsg::WindowAdjusted { .. } => {}
                    _ => {}
                }
            }
        }
    }

    let _ = session
        .disconnect(Disconnect::ByApplication, "", "English")
        .await;

    Ok(())
}

async fn connect_ssh(
    host: &str,
    port: u16,
    username: &str,
    auth: &str,
    key_path: &str,
    password: Option<&str>,
) -> Result<client::Handle<ClientHandler>, String> {
    let config = Arc::new(client::Config {
        nodelay: true,
        ..Default::default()
    });
    let handler = ClientHandler {
        host: host.to_string(),
        port,
    };

    let mut session = client::connect(config, (host, port), handler)
        .await
        .map_err(|error| format!("SSH 连接失败: {error}"))?;

    let result = if auth == "password" {
        session
            .authenticate_password(username, password.unwrap_or_default())
            .await
            .map_err(|error| format!("SSH 密码认证失败: {error}"))?
    } else {
        let key = load_secret_key(key_path, password).map_err(|error| match error {
            KeyError::KeyIsEncrypted => "私钥已加密，请填写 SSH 私钥口令".to_string(),
            other => format!("读取 SSH 私钥失败（可能是私钥口令错误）: {other}"),
        })?;
        let hash = session
            .best_supported_rsa_hash()
            .await
            .map_err(|error| format!("SSH 密钥协商失败: {error}"))?
            .flatten();
        session
            .authenticate_publickey(username, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
            .await
            .map_err(|error| format!("SSH 公钥认证失败: {error}"))?
    };

    if !result.success() {
        return Err("SSH 认证失败，请检查用户名、密码或私钥".to_string());
    }

    Ok(session)
}
