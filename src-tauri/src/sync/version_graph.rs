use std::collections::{HashMap, HashSet};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionNode {
    pub node_id: String,
    pub client_id: String,
    pub version_number: i64,
    pub content_hash: String,
    pub timestamp: DateTime<Utc>,
    pub file_size: i64,
    pub parent_hash: Option<String>,
    pub block_hashes: Vec<String>,
}

impl VersionNode {
    pub fn new(
        client_id: &str,
        version_number: i64,
        content_hash: &str,
        timestamp: DateTime<Utc>,
        file_size: i64,
        parent_hash: Option<String>,
        block_hashes: Vec<String>,
    ) -> Self {
        let node_id = format!("{}:{}", client_id, version_number);
        Self {
            node_id,
            client_id: client_id.to_string(),
            version_number,
            content_hash: content_hash.to_string(),
            timestamp,
            file_size,
            parent_hash,
            block_hashes,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConflictInfo {
    pub file_path: String,
    pub local_node_id: String,
    pub remote_node_id: String,
    pub local_client_id: String,
    pub remote_client_id: String,
    pub local_timestamp: DateTime<Utc>,
    pub remote_timestamp: DateTime<Utc>,
    pub resolved_winner: String,
    pub resolution_strategy: String,
    pub needs_manual_merge: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionGraph {
    pub file_path: String,
    pub nodes: HashMap<String, VersionNode>,
    pub child_edges: HashMap<String, HashSet<String>>,
    pub parent_edges: HashMap<String, String>,
    pub heads: HashSet<String>,
}

impl VersionGraph {
    pub fn new(file_path: &str) -> Self {
        Self {
            file_path: file_path.to_string(),
            nodes: HashMap::new(),
            child_edges: HashMap::new(),
            parent_edges: HashMap::new(),
            heads: HashSet::new(),
        }
    }

    pub fn add_node(&mut self, node: VersionNode) -> Vec<ConflictInfo> {
        let node_id = node.node_id.clone();
        let client_id = node.client_id.clone();
        let ts = node.timestamp;
        let parent_hash = node.parent_hash.clone();

        self.nodes.insert(node_id.clone(), node);

        if let Some(p_hash) = parent_hash {
            for (nid, nd) in &self.nodes {
                if nd.content_hash == p_hash && nid != &node_id {
                    self.parent_edges.insert(node_id.clone(), nid.clone());
                    self.child_edges
                        .entry(nid.clone())
                        .or_insert_with(HashSet::new)
                        .insert(node_id.clone());
                    self.heads.remove(nid);
                    break;
                }
            }
        }

        let client_nodes: Vec<String> = self
            .nodes
            .values()
            .filter(|n| n.client_id == client_id)
            .map(|n| n.node_id.clone())
            .collect();
        let mut has_child_from_same_client = false;
        for cnid in &client_nodes {
            if let Some(children) = self.child_edges.get(cnid) {
                for c in children {
                    if let Some(cnode) = self.nodes.get(c) {
                        if cnode.client_id == client_id {
                            has_child_from_same_client = true;
                            break;
                        }
                    }
                }
            }
        }
        if !has_child_from_same_client {
            self.heads.insert(node_id.clone());
        }

        let heads_from_different_clients: HashSet<String> = self
            .heads
            .iter()
            .filter(|h| {
                self.nodes
                    .get(*h)
                    .map(|n| n.client_id != client_id)
                    .unwrap_or(false)
            })
            .cloned()
            .collect();

        let mut conflicts = Vec::new();
        let my_head = self
            .heads
            .iter()
            .find(|h| {
                self.nodes
                    .get(*h)
                    .map(|n| n.client_id == client_id)
                    .unwrap_or(false)
            })
            .cloned();

        if let Some(my_h) = my_head {
            for other_h in &heads_from_different_clients {
                let my_node = self.nodes.get(&my_h).unwrap().clone();
                let other_node = self.nodes.get(other_h).unwrap().clone();

                let winner_client = if my_node.timestamp >= other_node.timestamp {
                    my_node.client_id.clone()
                } else {
                    other_node.client_id.clone()
                };

                conflicts.push(ConflictInfo {
                    file_path: self.file_path.clone(),
                    local_node_id: my_h.clone(),
                    remote_node_id: other_h.clone(),
                    local_client_id: my_node.client_id.clone(),
                    remote_client_id: other_node.client_id.clone(),
                    local_timestamp: my_node.timestamp,
                    remote_timestamp: other_node.timestamp,
                    resolved_winner: winner_client.clone(),
                    resolution_strategy: "LWW".to_string(),
                    needs_manual_merge: winner_client.is_empty(),
                });

                if my_node.timestamp < other_node.timestamp {
                    self.heads.remove(&my_h);
                } else if my_node.timestamp > other_node.timestamp {
                    self.heads.remove(other_h);
                }
            }
        }

        conflicts
    }

    pub fn resolve_conflict_lww(&self) -> Option<&VersionNode> {
        self.heads
            .iter()
            .filter_map(|h| self.nodes.get(h))
            .max_by_key(|n| n.timestamp)
    }

    pub fn get_missing_blocks(&self, local_hashes: &HashSet<String>) -> Vec<String> {
        let latest = self.resolve_conflict_lww();
        match latest {
            Some(node) => node
                .block_hashes
                .iter()
                .filter(|h| !local_hashes.contains(h.as_str()))
                .cloned()
                .collect(),
            None => Vec::new(),
        }
    }

    pub fn get_ancestors(&self, node_id: &str) -> Vec<&VersionNode> {
        let mut result = Vec::new();
        let mut visited = HashSet::new();
        let mut stack = vec![node_id.to_string()];
        while let Some(current) = stack.pop() {
            if !visited.insert(current.clone()) {
                continue;
            }
            if let Some(node) = self.nodes.get(&current) {
                result.push(&*node);
                if let Some(parent) = self.parent_edges.get(&current) {
                    stack.push(parent.clone());
                }
            }
        }
        result
    }

    pub fn get_all_block_hashes(&self) -> HashSet<String> {
        self.nodes
            .values()
            .flat_map(|n| n.block_hashes.iter().cloned())
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct VersionGraphStore {
    pub graphs: HashMap<String, VersionGraph>,
}

impl VersionGraphStore {
    pub fn new() -> Self {
        Self {
            graphs: HashMap::new(),
        }
    }

    pub fn get_or_create(&mut self, file_path: &str) -> &mut VersionGraph {
        self.graphs
            .entry(file_path.to_string())
            .or_insert_with(|| VersionGraph::new(file_path))
    }

    pub fn publish_version(&mut self, file_path: &str, node: VersionNode) -> Vec<ConflictInfo> {
        let graph = self.get_or_create(file_path);
        graph.add_node(node)
    }

    pub fn get_file_conflicts(&self, file_path: &str) -> Vec<ConflictInfo> {
        self.graphs
            .get(file_path)
            .map(|g| {
                let heads: Vec<&VersionNode> = g
                    .heads
                    .iter()
                    .filter_map(|h| g.nodes.get(h))
                    .collect();
                let mut conflicts = Vec::new();
                for i in 0..heads.len() {
                    for j in (i + 1)..heads.len() {
                        let a = heads[i];
                        let b = heads[j];
                        let winner = if a.timestamp >= b.timestamp {
                            a.client_id.clone()
                        } else {
                            b.client_id.clone()
                        };
                        conflicts.push(ConflictInfo {
                            file_path: file_path.to_string(),
                            local_node_id: a.node_id.clone(),
                            remote_node_id: b.node_id.clone(),
                            local_client_id: a.client_id.clone(),
                            remote_client_id: b.client_id.clone(),
                            local_timestamp: a.timestamp,
                            remote_timestamp: b.timestamp,
                            resolved_winner: winner,
                            resolution_strategy: "LWW".to_string(),
                            needs_manual_merge: false,
                        });
                    }
                }
                conflicts
            })
            .unwrap_or_default()
    }

    pub fn get_all_conflicts(&self) -> Vec<ConflictInfo> {
        let mut all = Vec::new();
        let paths: Vec<String> = self.graphs.keys().cloned().collect();
        for path in paths {
            all.extend(self.get_file_conflicts(&path));
        }
        all
    }
}
