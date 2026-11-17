//! The passive tree (đạo đồ, design section 17.4): nodes carrying modifiers or a keystone, linked into a graph a
//! fighter allocates from its start. Only the base exists for now: the tree is empty ([`Tree::default`]) and so is
//! every allocation, which is the fight as it is without a tree.

use crate::keystone::Keystone;
use crate::stat::Mod;

/// One node: its id, name, modifiers, the keystone it grants if it is one, and the nodes it links to.
#[derive(Clone, Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Node {
    pub id: u32,
    pub name: String,
    pub mods: Vec<Mod>,
    pub keystone: Option<Keystone>,
    pub links: Vec<u32>,
}

/// The whole tree and the nodes a fighter starts from.
#[derive(Clone, Debug, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Tree {
    pub nodes: Vec<Node>,
    pub starts: Vec<u32>,
}

/// Why an allocation is not allowed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeError {
    /// A node the tree does not have.
    Unknown(u32),
    /// A node no allocated path reaches from a start.
    Detached(u32),
    /// More nodes than points.
    TooMany { allocated: usize, points: usize },
}

impl Tree {
    pub fn node(&self, id: u32) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    fn linked(&self, a: u32, b: u32) -> bool {
        self.node(a).is_some_and(|n| n.links.contains(&b))
            || self.node(b).is_some_and(|n| n.links.contains(&a))
    }

    /// Whether `allocated` is a legal allocation of at most `points`: every node known and reached from a start
    /// through allocated nodes.
    pub fn check(&self, allocated: &[u32], points: usize) -> Result<(), TreeError> {
        if allocated.len() > points {
            return Err(TreeError::TooMany {
                allocated: allocated.len(),
                points,
            });
        }
        if let Some(&id) = allocated.iter().find(|&&id| self.node(id).is_none()) {
            return Err(TreeError::Unknown(id));
        }
        let mut reached: Vec<u32> = allocated
            .iter()
            .copied()
            .filter(|id| self.starts.contains(id))
            .collect();
        let mut grew = true;
        while grew {
            grew = false;
            for &id in allocated {
                if !reached.contains(&id) && reached.iter().any(|&r| self.linked(r, id)) {
                    reached.push(id);
                    grew = true;
                }
            }
        }
        match allocated.iter().find(|id| !reached.contains(id)) {
            Some(&id) => Err(TreeError::Detached(id)),
            None => Ok(()),
        }
    }

    /// Every modifier of the allocated nodes, in allocation order.
    pub fn mods<'a>(&'a self, allocated: &'a [u32]) -> impl Iterator<Item = &'a Mod> {
        allocated
            .iter()
            .filter_map(|&id| self.node(id))
            .flat_map(|n| n.mods.iter())
    }

    /// The keystones the allocated nodes grant.
    pub fn keystones(&self, allocated: &[u32]) -> Vec<Keystone> {
        allocated
            .iter()
            .filter_map(|&id| self.node(id).and_then(|n| n.keystone))
            .collect()
    }
}
