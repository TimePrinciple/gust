
use types::ObjectType;
use super::{hash::Hash, id::ID};
use sha1::{Digest, Sha1};
use std::{convert::TryFrom};
use super::Metadata;
const COMMIT_OBJECT_TYPE: &[u8] = b"commit";
const TREE_OBJECT_TYPE: &[u8] = b"tree";
const BLOB_OBJECT_TYPE: &[u8] = b"blob";
const TAG_OBJECT_TYPE: &[u8] = b"tag";
use super::hash::HASH_BYTES;
pub mod types;
pub mod delta;
//Object内存存储类型 
#[derive(Clone, Debug)]
pub struct Object {
  pub object_type: ObjectType,
  pub contents: Vec<u8>,
}
impl Object {
    /// object 的 hash转化函数
    pub fn hash(&self) -> Hash {
      let new_hash = Sha1::new()
        .chain(match self.object_type {
          ObjectType::Commit => COMMIT_OBJECT_TYPE,
          ObjectType::Tree => TREE_OBJECT_TYPE,
          ObjectType::Blob => BLOB_OBJECT_TYPE,
          ObjectType::Tag => TAG_OBJECT_TYPE,
        })
        .chain(b" ")
        .chain(self.contents.len().to_string())
        .chain(b"\0")
        .chain(&self.contents)
        .finalize();
      Hash(<[u8; HASH_BYTES]>::try_from(new_hash.as_slice()).unwrap())
    }
   // pub fn GetObjectFromPack()
    pub fn to_metadata(&self) -> Metadata{
      Metadata{
        t: self.object_type,
        h: super::hash::HashType::Sha1,
        id: ID::from_bytes(&self.hash().0),
        size: self.contents.len(),
        data: self.contents.clone(),
    }
    }
  }

