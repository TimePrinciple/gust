//!
//!
//！

use std::{collections::HashMap, path::Path};

use async_trait::async_trait;

use crate::git::{pack::Pack, protocol::RefCommand, errors::GitError, object::metadata::MetaData};

pub mod database;
pub mod fs;
pub mod structure;
pub mod utils;

pub const ZERO_ID: &'static str = match std::str::from_utf8(&[b'0'; 40]) {
    Ok(s) => s,
    Err(_) => panic!("can't get ZERO_ID"),
};

#[async_trait]
pub trait ObjectStorage: Clone + Send + Sync + std::fmt::Debug {
    async fn get_head_object_id(&self, path: &Path) -> String;

    async fn get_ref_object_id(&self, path: &Path) -> HashMap<String, String>;

    async fn handle_refs(&self, command: &RefCommand, path: &Path);

    async fn save_packfile(
        &self,
        decoded_pack: Pack,
        repo_path: &Path,
    ) -> Result<(), anyhow::Error>;

    async fn get_full_pack_data(&self, repo_path: &Path) -> Vec<u8>;

    async fn handle_pull_pack_data(&self) -> Vec<u8>;

    // get hash object from db if missing cache in unpack process
    async fn get_hash_object(&self, hash: &str) -> Result<MetaData, GitError>;
}
