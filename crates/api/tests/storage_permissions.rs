#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn newly_created_database_and_sidecars_are_private() {
    let directory = tempfile::tempdir().unwrap();
    let parent = directory.path().join("private");
    let database = parent.join("flo.db");
    let pool = flo_api::init_database(&format!("sqlite://{}", database.display()))
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(&parent).unwrap().permissions().mode() & 0o777,
        0o700
    );
    for name in ["flo.db", "flo.db-wal", "flo.db-shm"] {
        let file = parent.join(name);
        if file.exists() {
            assert_eq!(
                std::fs::metadata(file).unwrap().permissions().mode() & 0o777,
                0o600,
                "{name}"
            );
        }
    }
    pool.close().await;
}

#[tokio::test]
async fn custom_existing_database_permissions_are_not_silently_changed() {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("existing.db");
    std::fs::File::create(&database).unwrap();
    std::fs::set_permissions(&database, std::fs::Permissions::from_mode(0o640)).unwrap();
    let pool = flo_api::init_database(&format!("sqlite://{}", database.display()))
        .await
        .unwrap();
    assert_eq!(
        std::fs::metadata(database).unwrap().permissions().mode() & 0o777,
        0o640
    );
    pool.close().await;
}
