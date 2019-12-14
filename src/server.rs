use crate::db::{services::*, Database};

pub struct Keyserver {
    getter: MetadataGetter,
    putter: MetadataPutter,
}

impl Keyserver {
    pub fn new(db: Database) -> Self {
        let getter = MetadataGetter::new(db.clone());
        let putter = MetadataPutter::new(db);
        Keyserver { getter, putter }
    }
}
