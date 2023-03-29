//!
//!
//!
//!

use async_trait::async_trait;
use bytes::{BufMut, Bytes, BytesMut};
use russh::server::{Auth, Msg, Session};
use russh::*;
use russh_keys::*;
use std::collections::HashMap;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, BufReader};

use crate::git::protocol::ServiceType;
use crate::gust::driver::ObjectStorage;

use super::pack::{self};
use super::{PackProtocol, Protocol};

#[derive(Clone)]
pub struct SshServer<T: ObjectStorage> {
    pub client_pubkey: Arc<russh_keys::key::PublicKey>,
    pub clients: Arc<Mutex<HashMap<(usize, ChannelId), Channel<Msg>>>>,
    pub id: usize,
    pub storage: T,
    // is it a good choice to bind data here?
    pub pack_protocol: Option<PackProtocol<T>>,
}

impl<T: ObjectStorage> server::Server for SshServer<T> {
    type Handler = Self;
    fn new_client(&mut self, _: Option<std::net::SocketAddr>) -> Self {
        let s = self.clone();
        self.id += 1;
        s
    }
}

#[async_trait]
impl<T: ObjectStorage> server::Handler for SshServer<T> {
    type Error = anyhow::Error;

    async fn channel_open_session(
        self,
        channel: Channel<Msg>,
        session: Session,
    ) -> Result<(Self, bool, Session), Self::Error> {
        tracing::info!("SshServer::channel_open_session:{}", channel.id());
        {
            let mut clients = self.clients.lock().unwrap();
            clients.insert((self.id, channel.id()), channel);
        }
        Ok((self, true, session))
    }

    async fn exec_request(
        mut self,
        channel: ChannelId,
        data: &[u8],
        mut session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        let data = String::from_utf8_lossy(data).trim().to_owned();
        tracing::info!("exec: {:?},{}", channel, data);
        let res = self.handle_git_command(&data).await;
        session.data(channel, res.into());
        Ok((self, session))
    }

    async fn auth_publickey(
        self,
        user: &str,
        public_key: &key::PublicKey,
    ) -> Result<(Self, Auth), Self::Error> {
        tracing::info!("auth_publickey: {} / {:?}", user, public_key);
        Ok((self, server::Auth::Accept))
    }

    async fn auth_password(self, user: &str, password: &str) -> Result<(Self, Auth), Self::Error> {
        tracing::info!("auth_password: {} / {}", user, password);
        // in this example implementation, any username/password combination is accepted
        Ok((self, server::Auth::Accept))
    }

    async fn data(
        mut self,
        channel: ChannelId,
        data: &[u8],
        mut session: Session,
    ) -> Result<(Self, Session), Self::Error> {
        let pack_protocol = self.pack_protocol.as_mut().unwrap();
        let data_str = String::from_utf8_lossy(data).trim().to_owned();
        tracing::info!("data: {:?}, channel:{}", data_str, channel);
        match pack_protocol.service_type {
            Some(ServiceType::UploadPack) => {
                // let (send_pack_data, buf, pack_protocol) = self.handle_upload_pack(data).await;
                self.handle_upload_pack(channel, data, &mut session).await;
            }
            Some(ServiceType::ReceivePack) => {
                self.handle_receive_pack(channel, data, &mut session).await;
            }
            None => panic!(),
        };
        // session.eof(channel);
        // tracing::info!("send eof");
        // session.close(channel);
        Ok((self, session))
    }

    // async fn channel_eof(
    //     self,
    //     channel: ChannelId,
    //     mut session: Session,
    // ) -> Result<(Self, Session), Self::Error> {
    //     // let (self, session) = server::Handler::channel_eof(self, channel, session).await?;
    //     session.close(channel);
    //     Ok((self, session))
    // }

    // async fn channel_close(
    //     self,
    //     channel: ChannelId,
    //     session: Session,
    // ) -> Result<(Self, Session), Self::Error> {
    //     tracing::info!("channel_close: {:?}", channel);
    //     Ok((self, session))
    // }
}

impl<T: ObjectStorage> SshServer<T> {
    async fn handle_git_command(&mut self, command: &str) -> String {
        let command: Vec<_> = command.split(' ').collect();
        // command:
        // Push: git-receive-pack '/root/repotest/src.git'
        // Pull: git-upload-pack '/root/repotest/src.git'
        let path = command[1];
        let end = path.len() - ".git'".len();
        let mut pack_protocol = PackProtocol::new(
            PathBuf::from(&path[2..end]),
            command[0],
            Arc::new(self.storage.clone()),
            Protocol::Ssh,
        );
        let res = pack_protocol.git_info_refs().await;
        self.pack_protocol = Some(pack_protocol);
        String::from_utf8(res.to_vec()).unwrap()
    }

    async fn handle_upload_pack(&mut self, channel: ChannelId, data: &[u8], session: &mut Session) {
        let pack_protocol = self.pack_protocol.as_mut().unwrap();

        let (send_pack_data, buf) = pack_protocol
            .git_upload_pack(&mut Bytes::copy_from_slice(data))
            .await
            .unwrap();

        tracing::info!("buf is {:?}", buf);
        session.data(channel, String::from_utf8(buf.to_vec()).unwrap().into());

        let mut reader = BufReader::new(send_pack_data.as_slice());
        loop {
            let mut temp = BytesMut::new();
            let length = reader.read_buf(&mut temp).await.unwrap();
            if temp.is_empty() {
                let mut bytes_out = BytesMut::new();
                bytes_out.put_slice(pack::PKT_LINE_END_MARKER);
                session.data(channel, bytes_out.to_vec().into());
                return;
            }
            let bytes_out = pack_protocol.build_side_band_format(temp, length);
            tracing::info!("send: bytes_out: {:?}", bytes_out.clone().freeze());
            session.data(channel, bytes_out.to_vec().into());
        }
    }

    async fn handle_receive_pack(
        &mut self,
        channel: ChannelId,
        data: &[u8],
        session: &mut Session,
    ) {
        let pack_protocol = self.pack_protocol.as_mut().unwrap();

        let buf = pack_protocol
            .git_receive_pack(Bytes::from(data.to_vec()))
            .await
            .unwrap();
        session.data(channel, buf.to_vec().into());
    }
}
