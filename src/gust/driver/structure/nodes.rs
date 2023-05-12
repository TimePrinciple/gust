use std::{
    any::Any,
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use sea_orm::{ActiveValue::NotSet, Set};

use crate::{
    git::{
        hash::Hash,
        object::base::{
            blob::Blob,
            tree::{Tree, TreeItemType},
        },
        pack::decode::ObjDecodedMap,
    },
    gust::driver::{
        database::entity::node,
        utils::id_generator::{self, generate_id},
    },
};

use super::GitNodeObject;

pub struct Repo {
    // pub repo_root: Box<dyn Node>,
    pub tree_map: HashMap<Hash, Tree>,
    pub blob_map: HashMap<Hash, Blob>,
    pub tree_build_cache: HashSet<Hash>,
    // todo: limit the size of the cache
    // pub cache: LruCache<String, FileNode>,
}

pub struct TreeNode {
    pub nid: i64,
    pub pid: String,
    pub git_id: Hash,
    pub name: String,
    pub path: PathBuf,
    pub mode: Vec<u8>,
    pub children: Vec<Box<dyn Node>>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct FileNode {
    pub nid: i64,
    pub pid: String,
    pub git_id: Hash,
    pub name: String,
    pub path: PathBuf,
    pub mode: Vec<u8>,
    pub data: Vec<u8>,
}

/// define the node common behaviour
pub trait Node {
    fn get_id(&self) -> i64;

    fn get_pid(&self) -> &str;

    fn get_git_id(&self) -> Hash;

    fn get_name(&self) -> &str;

    fn get_mode(&self) -> Vec<u8>;

    fn get_children(&self) -> &Vec<Box<dyn Node>>;

    fn generate_id(&self) -> i64 {
        id_generator::generate_id()
    }

    fn new(name: String, pid: String) -> Self
    where
        Self: Sized;

    fn find_child(&mut self, name: &str) -> Option<&mut Box<dyn Node>>;

    fn add_child(&mut self, child: Box<dyn Node>);

    fn is_a_directory(&self) -> bool;

    fn as_any(&self) -> &dyn Any;

    // since we use lazy load, need manually fetch data, and might need to use a LRU cache to store the data?
    fn read_data(&self) -> String {
        "".to_string()
    }

    fn convert_to_model(&self) -> node::ActiveModel;

    // fn convert_from_model(node: node::Model, children: Vec<Box<dyn Node>>) -> Box<dyn Node>
    // where
    //     Self: Sized;
}

impl Node for TreeNode {
    fn get_id(&self) -> i64 {
        self.nid
    }
    fn get_pid(&self) -> &str {
        &self.pid
    }

    fn get_git_id(&self) -> Hash {
        self.git_id
    }
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_mode(&self) -> Vec<u8> {
        self.mode.clone()
    }

    fn get_children(&self) -> &Vec<Box<dyn Node>> {
        &self.children
    }

    fn new(name: String, pid: String) -> TreeNode {
        TreeNode {
            nid: generate_id(),
            pid,
            name,
            path: PathBuf::new(),
            mode: Vec::new(),
            git_id: Hash::default(),
            children: Vec::new(),
            data: Vec::new(),
        }
    }

    /// convert children relations to data vec
    fn convert_to_model(&self) -> node::ActiveModel {
        // tracing::info!("tree {}", Arc::strong_count(&self.data));
        // tracing::info!("tree {}", Arc::strong_count(&Arc::clone(&self.data)));
        node::ActiveModel {
            id: NotSet,
            node_id: Set(self.nid),
            git_id: Set(self.git_id.to_plain_str()),
            node_type: Set("tree".to_owned()),
            name: Set(self.name.to_string()),
            mode: Set(self.mode.clone()),
            content_sha: NotSet,
            data: Set(self.data.clone()),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        }
    }

    fn find_child(&mut self, name: &str) -> Option<&mut Box<dyn Node>> {
        self.children.iter_mut().find(|c| c.get_name() == name)
    }

    fn add_child(&mut self, content: Box<dyn Node>) {
        self.children.push(content);
    }

    fn is_a_directory(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    // fn convert_from_model(node: node::Model, children: Vec<Box<dyn Node>>) -> Box<dyn Node> {
    //     Box::new(TreeNode {
    //         nid: node.node_id,
    //         pid: node.pid,
    //         git_id: Hash::from_bytes(node.git_id.as_bytes()).unwrap(),
    //         name: node.name,
    //         path: PathBuf::new(),
    //         mode: node.mode,
    //         children,
    //         data: Vec::new(),
    //     })
    // }
}

impl Node for FileNode {
    fn get_id(&self) -> i64 {
        self.nid
    }

    fn get_pid(&self) -> &str {
        &self.pid
    }

    fn get_git_id(&self) -> Hash {
        self.git_id
    }
    fn get_name(&self) -> &str {
        &self.name
    }

    fn get_mode(&self) -> Vec<u8> {
        self.mode.clone()
    }

    fn get_children(&self) -> &Vec<Box<dyn Node>> {
        panic!("not supported")
    }

    fn new(name: String, pid: String) -> FileNode {
        FileNode {
            nid: generate_id(),
            pid,
            path: PathBuf::new(),
            name,
            git_id: Hash::default(),
            mode: Vec::new(),
            data: Vec::new(),
        }
    }

    fn convert_to_model(&self) -> node::ActiveModel {
        node::ActiveModel {
            id: NotSet,
            node_id: Set(self.nid),
            git_id: Set(self.git_id.to_plain_str()),
            node_type: Set("blob".to_owned()),
            name: Set(self.name.to_string()),
            mode: Set(self.mode.clone()),
            content_sha: NotSet,
            data: Set(self.data.clone()),
            created_at: Set(chrono::Utc::now().naive_utc()),
            updated_at: Set(chrono::Utc::now().naive_utc()),
        }
    }

    fn find_child(&mut self, _: &str) -> Option<&mut Box<dyn Node>> {
        panic!("not supported")
    }

    fn add_child(&mut self, _: Box<dyn Node>) {
        panic!("not supported")
    }

    fn is_a_directory(&self) -> bool {
        false
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    // fn convert_from_model(node: node::Model, _: Vec<Box<dyn Node>>) -> Box<dyn Node> {
    //     Box::new(FileNode {
    //         nid: node.node_id,
    //         pid: node.pid,
    //         git_id: Hash::from_bytes(node.git_id.as_bytes()).unwrap(),
    //         name: node.name,
    //         path: PathBuf::new(),
    //         mode: node.mode,
    //     })
    // }
}

impl TreeNode {
    // since root tree doesn't have name, we can only use node id to build it.
    pub fn get_root_from_nid(nid: i64) -> Box<dyn Node> {
        Box::new(TreeNode {
            nid,
            pid: "".to_owned(),
            git_id: Hash::default(),
            name: "".to_owned(),
            path: PathBuf::from("/"),
            mode: Vec::new(),
            children: Vec::new(),
            data: Vec::new(),
        })
    }
}

/// this method is used to build node tree and persist node data to database. Conversion order:
/// 1. Git TreeItem => Struct Node => DB Model
/// 2. Git Blob => DB Model
/// current: protocol => storage => structure
/// expected: protocol => structure => storage
pub async fn build_node_tree(
    result: &ObjDecodedMap,
    _: &Path,
) -> Result<Vec<node::ActiveModel>, anyhow::Error> {
    let tree_map: HashMap<Hash, Tree> = result
        .trees
        .clone()
        .into_iter()
        .map(|tree| (tree.meta.id, tree))
        .collect();

    let blob_map: HashMap<Hash, Blob> = result
        .blobs
        .clone()
        .into_iter()
        .map(|b| (b.meta.id, b))
        .collect();

    let mut repo = Repo {
        tree_map,
        blob_map,
        tree_build_cache: HashSet::new(),
    };

    let mut nodes = Vec::new();

    for commit in &result.commits {
        let commit_tree_id = commit.tree_id;
        let tree = &repo.tree_map.get(&commit_tree_id).unwrap().clone();
        let mut root_node = tree.convert_to_node(None);
        repo.build_node_tree(tree, &mut root_node);
        nodes.extend(repo.convert_node_to_model(root_node.as_ref(), 0));
        print!("--------------------------------");
    }
    Ok(nodes)
}

impl Repo {
    /// convert Git TreeItem => Struct Node and build node tree
    pub fn build_node_tree(&mut self, tree: &Tree, node: &mut Box<dyn Node>) {
        for item in &tree.tree_items {
            if let Some(_) = self.tree_build_cache.get(&item.id) {
                continue;
            }
            if item.item_type == TreeItemType::Tree {
                // repo_path.push(item.filename.clone());
                let tree = self.tree_map.get(&item.id).unwrap();
                node.add_child(tree.convert_to_node(Some(item)));
                let child_node = match node.find_child(&item.filename) {
                    Some(child) => child,
                    None => panic!("Something wrong!:{}", &item.filename),
                };
                let item = self.tree_map.get(&item.id);
                if let Some(item) = item {
                    self.build_node_tree(&item.clone(), child_node);
                }
                // repo_path.pop();
            } else {
                let blob = self.blob_map.get(&item.id).unwrap();
                node.add_child(blob.convert_to_node(Some(item)));
            }
            self.tree_build_cache.insert(item.id);
        }
    }

    /// conver Node to db entity and for later persistent
    pub fn convert_node_to_model(&self, node: &dyn Node, depth: u32) -> Vec<node::ActiveModel> {
        print_node(node, depth);
        let mut nodes: Vec<node::ActiveModel> = Vec::new();
        nodes.push(node.convert_to_model());
        if node.is_a_directory() {
            for child in node.get_children() {
                nodes.extend(self.convert_node_to_model(child.as_ref(), depth + 1));
            }
        }
        nodes
    }
}

// Model => Node => Tree ?
// pub fn model_to_node(nodes_model: &Vec<node::Model>, pid: &str) -> Vec<Box<dyn Node>> {
//     let mut nodes: Vec<Box<dyn Node>> = Vec::new();
//     for model in nodes_model {
//         if model.pid == pid {
//             if model.node_type == "blob" {
//                 nodes.push(FileNode::convert_from_model(model.clone(), Vec::new()));
//             } else {
//                 let childs = model_to_node(nodes_model, &model.pid);
//                 nodes.push(TreeNode::convert_from_model(model.clone(), childs));
//             }
//         }
//     }
//     nodes
// }

/// Print a node with format.
pub fn print_node(node: &dyn Node, depth: u32) {
    if depth == 0 {
        println!("{}", node.get_name());
    } else {
        println!(
            "{:indent$}└── {} {}",
            "",
            node.get_name(),
            node.get_id(),
            indent = ((depth as usize) - 1) * 4
        );
    }
}

#[cfg(test)]
mod test {
    use std::path::PathBuf;

    use crate::gust::driver::{
        database::entity::node,
        structure::nodes::{Node, TreeNode},
        utils::id_generator,
    };

    use super::FileNode;

    #[test]
    pub fn main() {
        // Form our INPUT:  a list of paths.
        let paths = vec![
            PathBuf::from("child1/grandchild1.txt"),
            PathBuf::from("child1/grandchild2.txt"),
            PathBuf::from("child2/grandchild3.txt"),
            PathBuf::from("child3"),
        ];
        println!("Input Paths:\n{:#?}\n", paths);
        id_generator::set_up_options().unwrap();
        // let mut root = init_root();
        // for path in paths.iter() {
        //     build_tree(&mut root, path, 0)
        // }

        // let mut save_models: Vec<node::ActiveModel> = Vec::new();

        // traverse_node(root.as_ref(), 0, &mut save_models);
    }

    fn build_tree(node: &mut Box<dyn Node>, path: &PathBuf, depth: usize) {
        let parts: Vec<&str> = path.to_str().unwrap().split("/").collect();

        if depth < parts.len() {
            let child_name = parts[depth];

            let child = match node.find_child(&child_name) {
                Some(child) => child,
                None => {
                    if path.is_file() {
                        node.add_child(Box::new(FileNode::new(
                            child_name.to_owned(),
                            "".to_owned(),
                        )));
                    } else {
                        node.add_child(Box::new(TreeNode::new(
                            child_name.to_owned(),
                            "".to_owned(),
                        )));
                    };
                    match node.find_child(&child_name) {
                        Some(child) => child,
                        None => panic!("Something wrong!:{}, {}", &child_name, depth),
                    }
                }
            };
            build_tree(child, path, depth + 1);
        }
    }
}
