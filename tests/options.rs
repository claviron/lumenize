use super::*;

/// Lumenize's keys and the runner's share one flat file, and a key neither
/// knows is refused rather than ignored.
#[test]
fn a_flat_file_divides_and_a_typo_is_refused() {
    let read: Options =
        serde_yaml::from_str("orbit: true\nconfig_dir: config/runtime\nmax_in_flight: 128\n")
            .unwrap();
    assert!(read.runner.orbit);
    assert_eq!(read.runner.max_in_flight, Some(128));
    assert_eq!(read.config_dir, PathBuf::from("config/runtime"));

    assert!(serde_yaml::from_str::<Options>("orbitt: true\n").is_err());
}

/// A worker is started with no command line, so what it serves has to
/// survive the envelope.
#[test]
fn a_launch_survives_the_envelope() {
    let sent: Options = serde_yaml::from_str(
        "name: shop\norbit: true\nworkers: 2\nrequests:\n  head_timeout: 60s\nconfig_dir: rt\n",
    )
    .unwrap();
    let back = Options::decode(&sent.encode().unwrap()).unwrap();
    assert_eq!(back.runner.name.as_deref(), Some("shop"));
    assert_eq!(back.runner.workers, Some(2));
    assert_eq!(back.config_dir, PathBuf::from("rt"));
    assert_eq!(
        back.runner.policy().unwrap().requests.head_timeout,
        Some(std::time::Duration::from_secs(60))
    );
}
