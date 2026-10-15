use rusqlite::{params, Connection, OpenFlags};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::Path;

pub struct Store {
    connection: Connection,
}
#[derive(Default)]
pub struct Records {
    pub genomes: Vec<GenomeRow>,
    pub creatures: Vec<CreatureRow>,
    pub mutations: Vec<MutationRow>,
    pub species: Vec<crate::Species>,
    pub replays: Vec<ReplayRow>,
}
pub struct GenomeRow {
    pub hash: Vec<u8>,
    pub blob: Vec<u8>,
    pub traits: Vec<u8>,
    pub viability: f32,
    pub tick: u64,
    pub parents: Vec<u8>,
    pub exploit: bool,
}
pub struct CreatureRow {
    pub id: u64,
    pub genome: Vec<u8>,
    pub born: u64,
    pub died: Option<u64>,
    pub species: u64,
    pub cause: Option<String>,
    pub offspring: u64,
    pub parents: Vec<u64>,
}
pub struct MutationRow {
    pub id: u64,
    pub tick: u64,
    pub record: bestiary_core::genome::MutationRecord,
}
pub struct ReplayRow {
    pub id: String,
    pub genome: Vec<u8>,
    pub data: Vec<u8>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct TimeSample {
    pub tick: u64,
    pub population: usize,
    pub species: usize,
    pub biomass: f64,
    pub trophic_biomass: [f64; 3],
    pub mean_traits: Vec<f32>,
    pub trait_variance: Vec<f32>,
    pub plant_toxicity: f32,
    pub herbivore_resistance: f32,
    pub coverage: f32,
}
pub trait Persistable: Serialize {
    fn tick(&self) -> u64;
    fn records(&self) -> Records;
    fn population(&self) -> usize;
    fn biomass(&self) -> f64;
    fn sample(&self) -> TimeSample;
}
type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;
impl Store {
    pub fn create(path: &Path) -> Result<Self> {
        if path.exists() {
            return Err("world file already exists".into());
        }
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let store = Self {
            connection: Connection::open(path)?,
        };
        store.schema()?;
        Ok(store)
    }
    pub fn open(path: &Path, read_only: bool) -> Result<Self> {
        let flags = if read_only {
            OpenFlags::SQLITE_OPEN_READ_ONLY
        } else {
            OpenFlags::SQLITE_OPEN_READ_WRITE
        };
        let store = Self {
            connection: Connection::open_with_flags(path, flags)?,
        };
        let version: i64 = store
            .connection
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        if version != 1 {
            return Err(format!("unsupported world schema {version}").into());
        }
        if !read_only {
            store.connection.execute_batch(
                "CREATE TABLE IF NOT EXISTS samples(tick INTEGER PRIMARY KEY,data BLOB)",
            )?;
        }
        Ok(store)
    }
    fn schema(&self) -> Result<()> {
        self.connection.execute_batch("
        PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; PRAGMA user_version=1;
        CREATE TABLE snapshot(id INTEGER PRIMARY KEY CHECK(id=1),tick INTEGER NOT NULL,blob BLOB NOT NULL);
        CREATE TABLE genome(hash BLOB PRIMARY KEY,blob BLOB,traits BLOB,viability REAL,discovery_time INTEGER,parents BLOB);
        CREATE TABLE creature(id INTEGER PRIMARY KEY,genome BLOB,born INTEGER,died INTEGER,species INTEGER,cause TEXT,offspring INTEGER,parents BLOB);
        CREATE TABLE species(id INTEGER PRIMARY KEY,name TEXT,founder BLOB,born INTEGER,extinct INTEGER,parent INTEGER,population INTEGER);
        CREATE TABLE phylogeny_edge(parent INTEGER,child INTEGER,PRIMARY KEY(parent,child));
        CREATE TABLE mutation(id INTEGER PRIMARY KEY,tick INTEGER,operator TEXT,parent BLOB,child BLOB,traits BLOB,viability REAL,born INTEGER,lifespan INTEGER,lineage_survival INTEGER);
        CREATE TABLE replay(id TEXT PRIMARY KEY,genome BLOB,data BLOB);
        CREATE TABLE event(tick INTEGER,sequence INTEGER,kind TEXT,data BLOB,PRIMARY KEY(tick,sequence));
        CREATE TABLE samples(tick INTEGER PRIMARY KEY,data BLOB);
        CREATE TABLE timeseries(tick INTEGER PRIMARY KEY,population INTEGER,biomass REAL,species INTEGER);
        CREATE TABLE archive_cell(cell BLOB PRIMARY KEY,genome BLOB,viability REAL);
        CREATE TABLE exploit_suspect(genome BLOB PRIMARY KEY,discovery_time INTEGER);
    ")?;
        Ok(())
    }
    pub fn load<T: DeserializeOwned>(&self) -> Result<T> {
        let bytes: Vec<u8> =
            self.connection
                .query_row("SELECT blob FROM snapshot WHERE id=1", [], |r| r.get(0))?;
        Ok(postcard::from_bytes(&bytes)?)
    }
    pub fn save<T: Persistable>(&mut self, world: &T) -> Result<()> {
        let records = world.records();
        let blob = postcard::to_allocvec(world)?;
        let transaction = self.connection.transaction()?;
        transaction.execute(
            "INSERT OR REPLACE INTO snapshot VALUES(1,?1,?2)",
            params![world.tick(), blob],
        )?;
        for row in records.genomes {
            transaction.execute(
                "INSERT OR IGNORE INTO genome VALUES(?1,?2,?3,?4,?5,?6)",
                params![
                    row.hash,
                    row.blob,
                    row.traits,
                    row.viability,
                    row.tick,
                    row.parents
                ],
            )?;
            if row.exploit {
                transaction.execute(
                    "INSERT OR IGNORE INTO exploit_suspect VALUES(?1,?2)",
                    params![row.hash, row.tick],
                )?;
            }
        }
        for r in records.creatures {
            transaction.execute(
                "INSERT OR REPLACE INTO creature VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![
                    r.id,
                    r.genome,
                    r.born,
                    r.died,
                    r.species,
                    r.cause,
                    r.offspring,
                    postcard::to_allocvec(&r.parents)?
                ],
            )?;
        }
        for s in &records.species {
            transaction.execute(
                "INSERT OR REPLACE INTO species VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![
                    s.id,
                    s.name,
                    s.founder.as_slice(),
                    s.born,
                    s.extinct,
                    s.parent,
                    s.population
                ],
            )?;
            if let Some(parent) = s.parent {
                transaction.execute(
                    "INSERT OR IGNORE INTO phylogeny_edge VALUES(?1,?2)",
                    params![parent, s.id],
                )?;
            }
        }
        for r in records.mutations {
            let m = r.record;
            transaction.execute(
                "INSERT OR REPLACE INTO mutation VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
                params![
                    r.id,
                    r.tick,
                    m.operator,
                    m.parent.as_slice(),
                    m.child.as_slice(),
                    postcard::to_allocvec(&m.traits)?,
                    m.viability,
                    m.born,
                    m.lifespan,
                    m.lineage_survival
                ],
            )?;
        }
        for r in records.replays {
            transaction.execute(
                "INSERT OR IGNORE INTO replay VALUES(?1,?2,?3)",
                params![r.id, r.genome, r.data],
            )?;
        }
        transaction.execute(
            "INSERT OR REPLACE INTO timeseries VALUES(?1,?2,?3,?4)",
            params![
                world.tick(),
                world.population(),
                world.biomass(),
                records.species.iter().filter(|s| s.population > 0).count()
            ],
        )?;
        transaction.execute(
            "INSERT OR REPLACE INTO samples VALUES(?1,?2)",
            params![world.tick(), postcard::to_allocvec(&world.sample())?],
        )?;
        transaction.commit()?;
        Ok(())
    }
    pub fn events<T: Serialize>(&self, tick: u64, events: &[T]) -> Result<()> {
        for (sequence, event) in events.iter().enumerate() {
            self.connection.execute(
                "INSERT OR IGNORE INTO event VALUES(?1,?2,?3,?4)",
                params![tick, sequence, "simulation", postcard::to_allocvec(event)?],
            )?;
        }
        Ok(())
    }
    pub fn replay(&self, id: &str) -> Result<Vec<u8>> {
        Ok(self
            .connection
            .query_row("SELECT data FROM replay WHERE id=?1", [id], |r| r.get(0))?)
    }
    pub fn event_history<T: DeserializeOwned>(&self) -> Result<Vec<(u64, T)>> {
        let mut query = self
            .connection
            .prepare("SELECT tick,data FROM event ORDER BY tick,sequence")?;
        let rows = query.query_map([], |r| Ok((r.get::<_, u64>(0)?, r.get::<_, Vec<u8>>(1)?)))?;
        let mut out = vec![];
        for row in rows {
            let (tick, data) = row?;
            out.push((tick, postcard::from_bytes(&data)?));
        }
        Ok(out)
    }
    pub fn samples(&self) -> Result<Vec<TimeSample>> {
        let mut query = self
            .connection
            .prepare("SELECT data FROM samples ORDER BY tick")?;
        let rows = query.query_map([], |r| r.get::<_, Vec<u8>>(0))?;
        let mut result = vec![];
        for row in rows {
            result.push(postcard::from_bytes(&row?)?);
        }
        Ok(result)
    }
    pub fn counts(&self) -> Result<serde_json::Value> {
        let mut map = serde_json::Map::new();
        for table in [
            "genome",
            "creature",
            "species",
            "mutation",
            "event",
            "archive_cell",
            "replay",
        ] {
            let count: i64 =
                self.connection
                    .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))?;
            map.insert(table.into(), count.into());
        }
        Ok(map.into())
    }
}
