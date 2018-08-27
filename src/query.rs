use coord::LatLonDeg;
use osmpbf::{DenseNode, Node, PrimitiveBlock, Relation, Way};
use regex::Regex;


pub trait Query {
    type BI;
    fn create_block_index(&self, &PrimitiveBlock) -> Option<Self::BI>;
    fn node_matches(&self, &Self::BI, node: &Node) -> bool;
    fn dense_node_matches(&self, &Self::BI, dnode: &DenseNode) -> bool;
    fn way_matches(&self, &Self::BI, way: &Way) -> bool;
    fn relation_matches(&self, &Self::BI, relation: &Relation) -> bool;
}

#[derive(Debug, Eq, PartialEq)]
pub enum QueryArgs {
    ValuePattern(String),
    KeyValue(String, String),
    Intersection(Box<(QueryArgs, QueryArgs)>),
}

#[derive(Debug)]
pub enum QueryKind {
    ValuePattern(ValuePatternQuery),
    KeyValue(KeyValueQuery),
    Intersection(Box<(QueryArgs, QueryArgs)>),
}

impl QueryArgs {
    pub fn compile(self) -> Result<QueryKind, String> {
        match self {
            QueryArgs::ValuePattern(pattern) => {
                Ok(QueryKind::ValuePattern(ValuePatternQuery::new(&pattern)?))
            },
            QueryArgs::KeyValue(k, v) => {
                Ok(QueryKind::KeyValue(KeyValueQuery::new(k, v)))
            },
            _ => {
                //TODO implement
                unimplemented!();
            },
        }
    }
}

#[derive(Debug)]
pub struct ValuePatternQuery {
    re: Regex,
}

impl ValuePatternQuery {
    pub fn new(pattern: &str) -> Result<Self, String> {
        let re = Regex::new(&pattern)
            .map_err(|e| format!("{}", e))?;
        Ok(ValuePatternQuery { re })
    }
}

impl Query for ValuePatternQuery {
    type BI = ();

    fn create_block_index(&self, _block: &PrimitiveBlock) -> Option<()> {
        Some(())
    }

    fn node_matches(&self, _: &(), node: &Node) -> bool {
        for (_key, val) in node.tags() {
            if self.re.is_match(val) {
                return true;
            }
        }
        return false;
    }

    fn dense_node_matches(&self, _: &(), dnode: &DenseNode) -> bool {
        for (_key, val) in dnode.tags() {
            if self.re.is_match(val) {
                return true;
            }
        }
        return false;
    }

    fn way_matches(&self, _: &(), way: &Way) -> bool {
        for (_key, val) in way.tags() {
            if self.re.is_match(val) {
                return true;
            }
        }
        return false;
    }

    fn relation_matches(&self, _: &(), relation: &Relation) -> bool {
        for (_key, val) in relation.tags() {
            if self.re.is_match(val) {
                return true;
            }
        }
        return false;
    }
}

#[derive(Debug)]
pub struct KeyValueQuery {
    key: String,
    value: String,
}

impl KeyValueQuery {
    pub fn new<S: Into<String>>(key: S, value: S) -> Self {
        KeyValueQuery {
            key: key.into(),
            value: value.into(),
        }
    }
}

impl Query for KeyValueQuery {
    type BI = (Vec<u32>, Vec<u32>);

    fn create_block_index(&self, block: &PrimitiveBlock) -> Option<(Vec<u32>, Vec<u32>)> {
        let mut key_indices = vec![];
        let mut value_indices = vec![];

        let key_bytes = self.key.as_bytes();
        let value_bytes = self.value.as_bytes();

        for (i, string) in block.raw_stringtable().iter().enumerate() {
            if string.as_slice() == key_bytes {
                key_indices.push(i as u32);
            }
            if string.as_slice() == value_bytes {
                value_indices.push(i as u32);
            }
        }

        if key_indices.is_empty() || value_indices.is_empty() {
            // No matches possible for this block
            return None;
        }

        key_indices.sort();
        value_indices.sort();

        Some((key_indices, value_indices))
    }

    fn node_matches(&self, bi: &Self::BI, node: &Node) -> bool {
        for (key, val) in node.raw_tags() {
            if bi.0.binary_search(&key).is_ok() && bi.1.binary_search(&val).is_ok() {
                return true;
            }
        }
        return false;
    }

    fn dense_node_matches(&self, _bi: &Self::BI, dnode: &DenseNode) -> bool {
        //TODO use raw tags
        for (key, val) in dnode.tags() {
            if key == self.key && val == self.value {
                return true;
            }
        }
        return false;
    }

    fn way_matches(&self, bi: &Self::BI, way: &Way) -> bool {
        for (key, val) in way.raw_tags() {
            if bi.0.binary_search(&key).is_ok() && bi.1.binary_search(&val).is_ok() {
                return true;
            }
        }
        return false;
    }

    fn relation_matches(&self, bi: &Self::BI, relation: &Relation) -> bool {
        for (key, val) in relation.raw_tags() {
            if bi.0.binary_search(&key).is_ok() && bi.1.binary_search(&val).is_ok() {
                return true;
            }
        }
        return false;
    }
}

pub fn find_query_matches<Q: Query>(
    block: &PrimitiveBlock,
    query: &Q,
    matches: &mut Vec<LatLonDeg>,
    way_node_ids: &mut Vec<i64>,
) {
    if let Some(block_index) = query.create_block_index(block) {
        for node in block.groups().flat_map(|g| g.nodes()) {
            if query.node_matches(&block_index, &node) {
                let pos = LatLonDeg::new(node.lat(), node.lon());
                matches.push(pos);
            }
        }

        for node in block.groups().flat_map(|g| g.dense_nodes()) {
            if query.dense_node_matches(&block_index, &node) {
                let pos = LatLonDeg::new(node.lat(), node.lon());
                matches.push(pos);
            }
        }

        for way in block.groups().flat_map(|g| g.ways()) {
            if query.way_matches(&block_index, &way) {
                way_node_ids.push(way.refs_slice()[0]);
            }
        }
    }
}
