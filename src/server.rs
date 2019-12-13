use crate::db::Database;

pub struct Keyserver {
    db: Database,
}

impl Keyserver {
    pub fn new(db: Database) -> Self {
        Keyserver { db }
    }
}
