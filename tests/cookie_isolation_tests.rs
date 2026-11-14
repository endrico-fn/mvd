#![allow(clippy::unwrap_used, clippy::expect_used)]

use mvd::security::cookie_vault::CookieVault;
use mvd::security::profile_guard::ProfileGuard;
use rusqlite::Connection;
use std::path::Path;

const MOCK_PROFILES_INI: &str = r#"
[General]
StartWithLastProfile=1
Version=2

[Profile0]
Name=work-corporate-01
IsRelative=1
Path=Profiles/a1b2c3d4.work-corporate-01
Default=1

[Profile1]
Name=work-dev-02
IsRelative=1
Path=Profiles/e5f6g7h8.work-dev-02

[Profile2]
Name=pbf03
IsRelative=1
Path=Profiles/x9y8z7w6.pbf03
"#;

#[test]
fn test_work_profiles_are_strictly_blocked_and_only_pbf03_allowed() {
    let root = Path::new("/home/porvata/.mozilla/firefox");

    // Attempting to load work-corporate-01 or work-dev-02 MUST fail hard
    assert!(
        ProfileGuard::resolve_sandbox_profile(root, MOCK_PROFILES_INI, "work-corporate-01")
            .is_err()
    );
    assert!(ProfileGuard::resolve_sandbox_profile(root, MOCK_PROFILES_INI, "work-dev-02").is_err());
    assert!(ProfileGuard::resolve_sandbox_profile(root, MOCK_PROFILES_INI, "default").is_err());

    // Only pbf03 succeeds
    let valid = ProfileGuard::resolve_sandbox_profile(root, MOCK_PROFILES_INI, "pbf03").unwrap();
    assert_eq!(valid.profile_name, "pbf03");
    assert!(valid
        .canonical_profile_dir
        .to_string_lossy()
        .contains("pbf03"));
}

#[test]
fn test_domain_scoped_cookie_filtering_prevents_cross_site_sandbox_leaks() {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "CREATE TABLE moz_cookies (
            host TEXT,
            path TEXT,
            isSecure INTEGER,
            expiry INTEGER,
            name TEXT,
            value TEXT
        );
        INSERT INTO moz_cookies VALUES ('.youtube.com', '/', 1, 1899999999, 'CONSENT', 'YES+1');
        INSERT INTO moz_cookies VALUES ('music.youtube.com', '/', 1, 1899999999, 'PREF', 'f6=400');
        INSERT INTO moz_cookies VALUES ('.lk21official.pics', '/', 1, 1899999999, 'sess_id', 'secret_lk21');
        INSERT INTO moz_cookies VALUES ('.reddit.com', '/', 1, 1899999999, 'reddit_session', 'secret_reddit');",
    )
    .unwrap();

    let yt_cookies = CookieVault::query_domain_scoped_cookies(&conn, "youtube.com").unwrap();
    assert_eq!(yt_cookies.len(), 2);
    assert!(yt_cookies.iter().all(|c| c.host.contains("youtube.com")));
}
