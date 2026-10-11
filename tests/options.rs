use super::*;

/// Lumenize's keys and the runner's share one flat file, and a key neither
/// knows is refused rather than ignored.
#[test]
fn a_flat_file_divides_and_a_typo_is_refused() {
    let read: Options = serde_yaml::from_str::<Launch>(
        "orbit: true\nfleet_name: example\nphp: {application: {script_path: application.php}}\ncache: {}\nconfig_parser: {max_depth: 8}\nmax_in_flight: 128\n",
    )
    .unwrap()
    .into();
    assert!(read.runner.orbit);
    assert_eq!(read.runner.max_in_flight, Some(128));
    assert_eq!(read.runner.fleet_name.as_deref(), Some("example"));
    assert_eq!(read.configuration.config_parser.max_depth, 8);
    let cache: claviron::cache::CacheConfig =
        serde_yaml::from_value(read.configuration.cache.unwrap()).unwrap();
    assert!(cache.is_empty());

    assert!(serde_yaml::from_str::<Launch>("orbitt: true\n").is_err());
    assert!(serde_yaml::from_str::<Launch>("config_dir: runtime\n").is_err());
}

/// A worker is started with no command line, so what it serves has to
/// survive the envelope — its name included, which it cannot read from an
/// empty command line.
#[test]
fn a_launch_survives_the_envelope() {
    let mut sent: Launch = serde_yaml::from_str(
        "orbit: true\nfleet_name: example\nworkers: 2\nrequests:\n  head_timeout: 60s\nconfig_path: deployment.yaml\nphp: {application: {script_path: application.php}}\nlogging: {path: logs/system.log}\nbroadcasting: {key: '${APP_KEY}', secret: '${APP_SECRET}'}\n",
    )
    .unwrap();
    let prepared = sent.prepare("example-app".into()).unwrap();
    assert_eq!(prepared.workers, 2);
    let back: Launch = serde_json::from_slice(&prepared.envelope).unwrap();
    assert_eq!(prepared.fleet_name.as_str(), "example");
    assert_eq!(back.runner.name.as_deref(), Some("example-app"));
    assert_eq!(back.runner.workers, None, "a worker starts no workers");
    assert_eq!(back.own.config_path, Some(PathBuf::from("deployment.yaml")));
    assert_eq!(back.own.broadcasting.unwrap()["secret"], "${APP_SECRET}");
    assert_eq!(back.own.logging.unwrap()["path"], "logs/system.log");
    assert_eq!(
        back.runner.policy().unwrap().requests.head_timeout,
        Some(std::time::Duration::from_secs(60))
    );
}

#[test]
fn single_system_log_does_not_require_channels_or_another_file() {
    let options: Options = serde_yaml::from_str::<Launch>(
        "logging: {path: logs/system.log, truncate: true}\n",
    )
    .unwrap()
    .into();
    let logging = options.system_log().unwrap().unwrap();
    assert_eq!(logging.path, PathBuf::from("logs/system.log"));
    assert_eq!(logging.format, claviron::logging::SystemLogFormat::Line);
    assert!(logging.truncate_on_startup);
}

#[test]
fn logging_rejects_misspelled_sections_and_non_mappings() {
    for raw in
        ["logging: {system_lgo: {path: logs/system.log}}", "logging: false"]
    {
        let options: Options =
            serde_yaml::from_str::<Launch>(raw).unwrap().into();
        assert!(options.system_log().is_err());
    }
}

#[test]
fn database_launch_survives_workers_without_resolving_environment() {
    let mut launch: Launch = serde_yaml::from_str(
        "orbit: true\nfleet_name: example\nworkers: 2\ndatabase:\n  postgres:\n    example:\n      url: '${DATABASE_URL}'\n      pool: {pool_size: 2, idle_size: 0, acquire_timeout: 1s, idle_timeout: 1s, max_lifetime: 1s}\n",
    )
    .unwrap();
    let sent = launch.prepare("example-app".into()).unwrap();
    let received: Launch = serde_json::from_slice(&sent.envelope).unwrap();
    assert_eq!(received.own.database, launch.own.database);
    assert_eq!(
        received.own.database.unwrap()["postgres"]["example"]["url"],
        "${DATABASE_URL}",
    );
}

#[cfg(feature = "postgres")]
#[test]
fn database_section_uses_the_existing_native_schema() {
    let configuration: Configuration = serde_yaml::from_str(
        "database:\n  postgres:\n    example:\n      url: 'postgresql://example@example.test/example?options=-csearch_path%3Dexample'\n      pool: {pool_size: 2, idle_size: 0, acquire_timeout: 1s, idle_timeout: 1s, max_lifetime: 1s}\n",
    )
    .unwrap();
    configuration.validate_database_features().unwrap();
    let config: claviron::db::DbFileConfig =
        serde_yaml::from_value(configuration.database.unwrap()).unwrap();
    let entry = config.postgres.get("example").unwrap();
    assert_eq!(entry.pool.pool_size, 2);
    assert_eq!(
        entry.url.as_ref().unwrap().query(),
        Some("options=-csearch_path%3Dexample")
    );
}

#[test]
fn database_requires_a_mapping_and_selected_backends() {
    let configuration: Configuration =
        serde_yaml::from_str("database: true").unwrap();
    assert!(configuration.validate_database_features().is_err());
    for (backend, enabled) in [
        ("postgres", cfg!(feature = "postgres")),
        ("mysql", cfg!(feature = "mysql")),
        ("sqlite", cfg!(feature = "sqlite")),
    ] {
        let configuration: Configuration =
            serde_yaml::from_str(&format!("database: {{{backend}: {{}}}}"))
                .unwrap();
        assert_eq!(configuration.validate_database_features().is_ok(), enabled);
    }
}
