//!
//!
//!
//!
//!
//!
//!

use std::fmt::Display;

use bstr::ByteSlice;

use crate::errors::GitError;
use crate::git::id::ID;
use crate::git::{Metadata, Type};
use crate::git::hash::Hash;

///
#[derive(PartialEq, Eq, Hash, Ord, PartialOrd, Debug, Clone, Copy)]
pub enum TreeItemType {
    Blob,
    BlobExecutable,
    Tree,
    Commit,
    Link,
}

///
impl Display for TreeItemType {
    ///
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            TreeItemType::Blob => write!(f, "blob"),
            TreeItemType::BlobExecutable => write!(f, "blob executable"),
            TreeItemType::Tree => write!(f, "tree"),
            TreeItemType::Commit => write!(f, "commit"),
            TreeItemType::Link => write!(f, "link"),
        }
    }
}

///
impl TreeItemType {
    ///
    #[allow(unused)]
    pub(crate) fn to_bytes(self) -> &'static [u8] {
        match self {
            TreeItemType::Blob => b"100644",
            TreeItemType::BlobExecutable => b"100755",
            TreeItemType::Tree => b"40000",
            TreeItemType::Link => b"120000",
            TreeItemType::Commit => b"160000",
        }
    }

    ///
    #[allow(unused)]
    pub(crate) fn tree_item_type_from(mode: &[u8]) -> Result<TreeItemType, GitError> {
        Ok(match mode {
            b"40000" => TreeItemType::Tree,
            b"100644" => TreeItemType::Blob,
            b"100755" => TreeItemType::BlobExecutable,
            b"120000" => TreeItemType::Link,
            b"160000" => TreeItemType::Commit,
            b"100664" => TreeItemType::Blob,
            b"100640" => TreeItemType::Blob,
            _ => return Err(GitError::InvalidTreeItem(String::from_utf8(mode.to_vec()).unwrap())),
        })
    }
}

/// Git Object: tree item
pub struct TreeItem {
    pub mode: Vec<u8>,
    pub item_type: TreeItemType,
    pub id: ID,
    pub filename: String,
}

/// Git Object: tree
pub struct Tree {
    pub meta: Metadata,
    pub tree_items: Vec<TreeItem>,
}

///
impl Display for Tree {
    #[allow(unused)]
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        for item in &self.tree_items {
            writeln!(f, "{} {} {} {}",
                     String::from_utf8(item.mode.to_vec()).unwrap(),
                     item.item_type, item.id, item.filename);
        }

        Ok(())
    }
}

///
impl Tree {
    ///
    #[allow(unused)]
    pub(crate) fn decode_metadata(&mut self) -> Result<(), GitError> {
        let mut tree_items:Vec<TreeItem> = Vec::new();
        let mut index = 0;

        while index < self.meta.data.len() {
            let mode_index = &self.meta.data[index..].find_byte(0x20).unwrap();
            let mode = &self.meta.data[index..index + *mode_index];
            let item_type = TreeItemType::tree_item_type_from(mode).unwrap();

            let filename_index = &self.meta.data[index..].find_byte(0x00).unwrap();
            let filename = String::from_utf8(self.meta.data[index + mode_index + 1.. index + *filename_index]
                .to_vec())
                .unwrap();

            let id = ID::from_bytes(&self.meta.data[index + filename_index + 1..index + filename_index + 21]);

            self.tree_items.push(TreeItem {
                mode: mode.to_vec(),
                item_type,
                id,
                filename,
            });

            index = index + filename_index + 21;
        }

        Ok(())
    }

    ///
    #[allow(unused)]
    pub(crate) fn encode_metadata(&self) -> Result<Metadata, ()> {
        let mut data = Vec::new();
        for item in &self.tree_items {
            data.extend_from_slice(&item.mode);
            data.extend_from_slice(0x20u8.to_be_bytes().as_ref());
            data.extend_from_slice(item.filename.as_bytes());
            data.extend_from_slice(0x00u8.to_be_bytes().as_ref());
            data.extend_from_slice(&item.id.bytes);
        }

        Ok(
            Metadata {
                t: Type::Tree,
                h: Hash::Sha1,
                id: ID::from_vec(Type::Tree, &mut data),
                size: data.len(),
                data,
            },
        )
    }

    ///
    #[allow(unused)]
    pub(crate) fn write_to_file(&self, root_path: String) -> Result<String, GitError> {
        self.meta.write_to_file(root_path)
    }
}

///
#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;
    use std::path::PathBuf;

    ///
    #[test]
    fn test_tree_write_to_file() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/blob-82352c3a6a7a8bd32011751699c7a3648d1b5d3c-gitmega.md");

        let meta =
            super::Metadata::read_object_from_file(path.to_str().unwrap().to_string())
                .expect("Read error!");

        assert_eq!(meta.t, super::Type::Blob);
        assert_eq!("82352c3a6a7a8bd32011751699c7a3648d1b5d3c", meta.id.to_string());
        assert_eq!(16, meta.size);

        let blob = crate::git::blob::Blob {
            meta: meta.clone(),
            data: meta.data,
        };

        assert_eq!(
            "# Hello Gitmega\n",
            String::from_utf8(blob.clone().data).unwrap().as_str()
        );

        let item = blob
            .to_tree_item(String::from("gitmega.md")).unwrap();

        let mut tree = super::Tree {
            meta: super::Metadata {
                t: super::Type::Tree,
                h: super::Hash::Sha1,
                id: super::ID {
                    bytes: vec![],
                    hash: String::new(),
                },
                size: 0,
                data: vec![]
            },
            tree_items: vec![item],
        };

        tree.meta = tree.encode_metadata().unwrap();
        tree.write_to_file("/tmp".to_string()).expect("Write error!");

        assert!(Path::new("/tmp/1b/dbc1e723aa199e83e33ecf1bb19f874a56ebc3").exists());
    }

    ///
    #[test]
    fn test_tree_write_to_file_2_blob() {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/blob-fc1a505ac94f98cc5f29100a2d9aef97027a32fb-gitmega.md");

        let meta_gitmega =
            super::Metadata::read_object_from_file(path.to_str().unwrap().to_string())
                .expect("Read error!");

        let blob_gitmega = crate::git::blob::Blob {
            meta: meta_gitmega.clone(),
            data: meta_gitmega.data,
        };

        let item_gitmega = blob_gitmega
            .to_tree_item(String::from("gitmega.md")).unwrap();

        path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/blob-a3b55a2ce16d2429dae2d690d2c15bcf26fbe33c-gust.md");

        let meta_gust =
            super::Metadata::read_object_from_file(path.to_str().unwrap().to_string())
                .expect("Read error!");

        let blob_gust = crate::git::blob::Blob {
            meta: meta_gust.clone(),
            data: meta_gust.data,
        };

        let item_gust = blob_gust
            .to_tree_item(String::from("gust.md")).unwrap();


        let mut tree = super::Tree {
            meta: super::Metadata {
                t: super::Type::Tree,
                h: super::Hash::Sha1,
                id: super::ID {
                    bytes: vec![],
                    hash: String::new(),
                },
                size: 0,
                data: vec![]
            },
            tree_items: vec![item_gitmega, item_gust],
        };

        tree.meta = tree.encode_metadata().unwrap();
        tree.write_to_file("/tmp".to_string()).expect("Write error!");

        assert!(Path::new("/tmp/9b/be4087bedef91e50dc0c1a930c1d3e86fd5f20").exists());
    }

    ///
    #[test]
    fn test_tree_read_from_file() {
        // 100644 blob 82352c3a6a7a8bd32011751699c7a3648d1b5d3c	gitmega.md
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/tree-1bdbc1e723aa199e83e33ecf1bb19f874a56ebc3");

        let meta = super::Metadata::read_object_from_file(path.to_str().unwrap().to_string())
            .expect("Read error!");

        assert_eq!(super::Type::Tree, meta.t);
        assert_eq!(38, meta.size);

        let mut tree = super::Tree {
            meta,
            tree_items: Vec::new(),
        };

        tree.decode_metadata().unwrap();

        assert_eq!(1, tree.tree_items.len());
        assert_eq!(
            "gitmega.md",
            tree.tree_items[0].filename.as_str()
        );
        assert_eq!(
            "82352c3a6a7a8bd32011751699c7a3648d1b5d3c",
            tree.tree_items[0].id.to_string()
        );
        assert_eq!(
            "100644",
            String::from_utf8(tree.tree_items[0].mode.to_vec()).unwrap().as_str()
        );
        assert_eq!(super::TreeItemType::Blob, tree.tree_items[0].item_type);
    }

    ///
    #[test]
    fn test_tree_read_from_file_2_items() {
        // 100644 blob fc1a505ac94f98cc5f29100a2d9aef97027a32fb	gitmega.md
        // 100644 blob a3b55a2ce16d2429dae2d690d2c15bcf26fbe33c	gust.md
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("resources/data/test/tree-9bbe4087bedef91e50dc0c1a930c1d3e86fd5f20");

        let meta = super::Metadata::read_object_from_file(path.to_str().unwrap().to_string())
            .expect("Read error!");

        assert_eq!(super::Type::Tree, meta.t);
        assert_eq!(73, meta.size);

        let mut tree = super::Tree {
            meta,
            tree_items: Vec::new(),
        };

        tree.decode_metadata().unwrap();

        assert_eq!(2, tree.tree_items.len());

        assert_eq!(
            "gitmega.md",
            tree.tree_items[0].filename.as_str()
        );
        assert_eq!(
            "fc1a505ac94f98cc5f29100a2d9aef97027a32fb",
            tree.tree_items[0].id.to_string()
        );
        assert_eq!(
            "100644",
            String::from_utf8(tree.tree_items[0].mode.to_vec()).unwrap().as_str()
        );
        assert_eq!(super::TreeItemType::Blob, tree.tree_items[0].item_type);

        assert_eq!(
            "gust.md",
            tree.tree_items[1].filename.as_str()
        );
        assert_eq!(
            "a3b55a2ce16d2429dae2d690d2c15bcf26fbe33c",
            tree.tree_items[1].id.to_string()
        );
        assert_eq!(
            "100644",
            String::from_utf8(tree.tree_items[1].mode.to_vec()).unwrap().as_str()
        );
        assert_eq!(super::TreeItemType::Blob, tree.tree_items[1].item_type);
    }
}