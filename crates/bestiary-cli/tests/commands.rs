use std::process::Command;
fn cli(args: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).unwrap()
}
#[test]
fn fresh_world_status_genome_combine_replay_and_phylogeny_commands() {
    let dir = std::env::temp_dir().join(format!("bestiary-cli-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("world.db");
    let replay = dir.join("sprint.rpl");
    let path = path.to_str().unwrap();
    let replay = replay.to_str().unwrap();
    let initial = cli(&[
        "world",
        "new",
        "--seed",
        "9",
        "--population",
        "4",
        "--out",
        path,
        "--json",
    ]);
    assert_eq!(initial["population"], 4);
    assert_eq!(
        cli(&["world", "status", path, "--json"])["state_hash"],
        initial["state_hash"]
    );
    assert!(cli(&["world", "attach", path, "--json"])["endpoint"].is_string());
    assert_eq!(
        cli(&["genome", "validate", "serpent", "--json"])["valid"],
        true
    );
    let result = cli(&["combine", "grazer", "--replay", replay, "--json"]);
    assert_eq!(result["exploit_suspect"], false);
    assert!(
        cli(&["replay", replay, "--json"])["frames"]
            .as_u64()
            .unwrap()
            > 1
    );
    assert!(cli(&["phylo", path, "--json"]).is_object());
    assert!(!Command::new(env!("CARGO_BIN_EXE_bestiary"))
        .arg("invalid")
        .output()
        .unwrap()
        .status
        .success());
    std::fs::remove_dir_all(dir).unwrap();
}
