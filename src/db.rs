use crate::{config::Config, generate_sonnet::Sonnet};
use anyhow::{Result, anyhow};
use log::info;

use std::fs;

use rusqlite::{Connection, named_params};

const SCHEMA_SQL: &str = include_str!("../schema.sql");

pub struct Db {
    pub conn: Connection,
}

impl Db {
    pub fn init_db(conf: &Config) -> Result<Self> {
        let db_path = &conf.db_path;
        let db_folder = match db_path.parent() {
            Some(p) => p,
            None => return Err(anyhow!("Could not determine DB path, please check config.")),
        };

        // Try to create the final dir (this is idempotent so it's chill to run every time)
        match fs::create_dir_all(&db_folder) {
            Ok(_) => (),
            Err(e) => {
                return Err(anyhow!(
                    "Could not create db dir at {:#?}: {}",
                    &db_folder,
                    e
                ));
            }
        }

        let conn = match Connection::open(&db_path) {
            Ok(c) => c,
            Err(e) => return Err(anyhow!("Could not establish connection with the db: {}", e)),
        };

        match conn.execute_batch(SCHEMA_SQL) {
            Ok(_) => (),
            Err(e) => return Err(anyhow!("Could not write to database: {}", e)),
        }

        info!("Database loaded.");
        Ok(Db { conn })
    }

    pub fn write_sonnet(&self, sonnet: &Sonnet) -> Result<()> {
        self.conn.execute(
            "INSERT INTO sonnets (author, content, created_at, noun, prompt)
             VALUES (:author, :content, :created_at, :noun, :prompt)",
            named_params! {
                ":author": sonnet.author,
                ":content": sonnet.content,
                ":created_at": sonnet.created_at,
                ":noun": sonnet.noun,
                ":prompt": sonnet.prompt,
            }
        )?;
        info!("Successfully saved sonnet to database.");
        Ok(())
    }
}
