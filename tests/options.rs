use super::*;

/// Lumenize's keys and the runner's share one flat file, and a key neither
/// knows is refused rather than ignored.
#[test]
fn a_flat_file_divides_and_a_typo_is_refused() {
    let read: Options = serde_yaml::from_str::<Launch>(
        "orbit: true\nconfig_dir: config/runtime\nmax_in_flight: 128\n",
    )
    .unwrap()
    .into();
    assert!(read.runner.orbit);
    assert_eq!(read.runner.max_in_flight, Some(128));
    assert_eq!(read.config_dir, PathBuf::from("config/runtime"));

    assert!(serde_yaml::from_str::<Launch>("orbitt: true\n").is_err());
}

/// A worker is started with no command line, so what it serves has to
/// survive the envelope — its name included, which it cannot read from an
/// empty command line.
#[test]
fn a_launch_survives_the_envelope() {
    let mut sent: Launch = serde_yaml::from_str(
        "orbit: true\nworkers: 2\nrequests:\n  head_timeout: 60s\nconfig_dir: rt\n",
    )
    .unwrap();
    let prepared = sent.prepare("shop".into()).unwrap();
    assert_eq!(prepared.workers, 2);
    let back: Launch = serde_json::from_slice(&prepared.envelope).unwrap();
    assert_eq!(back.runner.name.as_deref(), Some("shop"));
    assert_eq!(back.runner.workers, None, "a worker starts no workers");
    assert_eq!(back.own.config_dir, Some(PathBuf::from("rt")));
    assert_eq!(
        back.runner.policy().unwrap().requests.head_timeout,
        Some(std::time::Duration::from_secs(60))
    );
}
