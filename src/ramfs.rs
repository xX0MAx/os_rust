extern crate alloc;

use alloc::{rc::Rc, string::String, collections::BTreeMap, vec::Vec};
use core::cell::RefCell;

pub type NodeRef = Rc<RefCell<Node>>;

pub static mut ROOT_DIR: Option<NodeRef> = None;
pub static mut CURRENT_DIR: Option<NodeRef> = None;

#[derive(Debug)]
pub enum Node {
    Directory {
        entries: BTreeMap<String, NodeRef>,
    },
    File {
        content: Vec<u8>,
    },
}

impl Node {
    pub fn new_dir() -> NodeRef {
        Rc::new(RefCell::new(Node::Directory {
            entries: BTreeMap::new(),
        }))
    }

    pub fn new_file() -> NodeRef {
        Rc::new(RefCell::new(Node::File {
            content: Vec::new(),
        }))
    }

    pub fn add_entry(dir: &NodeRef, name: String, node: NodeRef) -> Result<(), &'static str> {
        let mut dir_borrow = dir.borrow_mut();
        match &mut *dir_borrow {
            Node::Directory { entries } => {
                if entries.contains_key(&name) {
                    return Err("Entry already exists");
                }
                entries.insert(name, node);
                Ok(())
            }
            _ => Err("Not a directory"),
        }
    }

    pub fn get_entry(dir: &NodeRef, name: &str) -> Option<NodeRef> {
        let dir_borrow = dir.borrow();
        match &*dir_borrow {
            Node::Directory { entries } => entries.get(name).cloned(),
            _ => None,
        }
    }

    pub fn write_file(file: &NodeRef, data: &[u8]) -> Result<(), &'static str> {
        let mut file_borrow = file.borrow_mut();
        match &mut *file_borrow {
            Node::File { content } => {
                content.clear();
                content.extend_from_slice(data);
                Ok(())
            }
            _ => Err("Not a file"),
        }
    }

    pub fn read_file(file: &NodeRef) -> Result<Vec<u8>, &'static str> {
        let file_borrow = file.borrow();
        match &*file_borrow {
           Node::File { content } => Ok(content.clone()),
           _ => Err("Not a file"),
        }
    }

    pub fn init_fs() {
        unsafe {
            let root = Node::new_dir();
            ROOT_DIR = Some(Node::new_dir());
            CURRENT_DIR = Some(root);
        }
    }

    pub fn remove_entry(dir: &NodeRef, name: &str) -> Result<(), &'static str> {
        let mut dir_borrow = dir.borrow_mut();
        match &mut *dir_borrow {
            Node::Directory { entries } => {
                if entries.remove(name).is_some() {
                    Ok(())
                } else {
                    Err("Entry not found")
                }
            }
            _ => Err("Not a directory"),
        }
    }
}