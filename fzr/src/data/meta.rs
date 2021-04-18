#[derive(Clone, DagCbor, Debug, Eq, PartialEq)]
pub enum MetadataRelationship {
    Is,
    Has,
}

#[derive(Clone, DagCbor, Debug, Eq, PartialEq)]
pub enum MetadataCategory {
    Originator,
    Attribute,
    Relation(MetadataRelationship),
}

#[derive(Clone, DagCbor, Debug, Eq, PartialEq)]
pub enum MetadataItem {
    parent: Option<MetadataItem>
    value: String
    category: MetadataCategory,
}