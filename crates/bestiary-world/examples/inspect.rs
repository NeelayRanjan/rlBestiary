use bestiary_archive::store::Store;
use bestiary_world::{EcologyEvent, World};
fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let path = std::env::args().nth(1).ok_or("pass world database")?;
    let store = Store::open(std::path::Path::new(&path), true)?;
    let w: World = store.load()?;
    let events = store.event_history::<EcologyEvent>()?;
    let first_fragmentation = events.iter().find_map(|(tick, event)| {
        matches!(event, EcologyEvent::Fragmentation { .. }).then_some(*tick)
    });
    let species = w
        .registry
        .species
        .values()
        .filter(|s| s.population > 0)
        .count();
    let splits = w
        .registry
        .species
        .values()
        .filter(|s| s.parent.is_some() && first_fragmentation.is_some_and(|t| s.born > t))
        .count();
    let max_parts = w
        .genomes
        .values()
        .filter(|g| !g.exploit)
        .map(|g| g.traits.parts)
        .max();
    println!("tick={} population={} species={} splits_after_fragmentation={} first_fragmentation={:?} max_valid_parts={:?} conserved={} mutations={}",w.tick,w.agents.len(),species,splits,first_fragmentation,max_parts,w.conserved(),w.mutations.len());
    Ok(())
}
