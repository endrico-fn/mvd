#![allow(clippy::unwrap_used, clippy::expect_used)]

use mvd::domain::media::{MediaArchetype, PlatformKind};
use mvd::engine::parser::UrlParser;

#[test]
fn test_all_seven_mandatory_urls_and_social_platforms() {
    // T-01: Video Playlist
    let t01 = UrlParser::parse_and_normalize(
        "https://youtube.com/playlist?list=PLIhvC56v63IKioClkSNDjW7iz-6TFvLwS&si=Me_WJsw1pF2svME5",
    )
    .unwrap();
    assert_eq!(t01.platform, PlatformKind::YouTube);
    assert_eq!(t01.initial_archetype, MediaArchetype::VideoPlaylist);
    assert!(!t01.normalized_url.contains("si="));
    assert_eq!(
        t01.extracted_id_hint.as_deref(),
        Some("PLIhvC56v63IKioClkSNDjW7iz-6TFvLwS")
    );

    // T-02: Video Single
    let t02 =
        UrlParser::parse_and_normalize("https://youtu.be/SPwyp2NG-bE?si=0d_ISKZOBA72UU3C").unwrap();
    assert_eq!(t02.platform, PlatformKind::YouTube);
    assert_eq!(t02.initial_archetype, MediaArchetype::SingleVideo);
    assert_eq!(t02.extracted_id_hint.as_deref(), Some("SPwyp2NG-bE"));

    // T-03: Music Album Artist
    let t03 = UrlParser::parse_and_normalize(
        "https://music.youtube.com/playlist?list=OLAK5uy_kXEi_1ry11kHAe71Z36AShGLhVXejbK38&si=s72KLYDSaQ7vXc-d&drli=1",
    )
    .unwrap();
    assert_eq!(t03.platform, PlatformKind::YouTubeMusic);
    assert_eq!(t03.initial_archetype, MediaArchetype::AudioPlaylist);
    assert!(!t03.normalized_url.contains("si="));
    assert!(t03.normalized_url.contains("drli=1"));

    // T-04: Music 1 (VEVO Age-Gated)
    let t04 = UrlParser::parse_and_normalize(
        "https://music.youtube.com/watch?v=LrSX_OcpeJg&si=3_UGDgDjz88J6h6J&drli=1",
    )
    .unwrap();
    assert_eq!(t04.platform, PlatformKind::YouTubeMusic);
    assert_eq!(t04.initial_archetype, MediaArchetype::SingleAudio);
    assert_eq!(t04.extracted_id_hint.as_deref(), Some("LrSX_OcpeJg"));

    // T-05: Music 2 (Art Track)
    let t05 = UrlParser::parse_and_normalize(
        "https://music.youtube.com/watch?v=5xx9gTEQaiQ&si=k20r8T991nBzeW0t&drli=1",
    )
    .unwrap();
    assert_eq!(t05.platform, PlatformKind::YouTubeMusic);
    assert_eq!(t05.initial_archetype, MediaArchetype::SingleAudio);
    assert_eq!(t05.extracted_id_hint.as_deref(), Some("5xx9gTEQaiQ"));

    // T-06: Music Community Playlist
    let t06 = UrlParser::parse_and_normalize(
        "https://music.youtube.com/playlist?list=PLJNrcbVaS76JVPdSmjIs2Iep9C7XBUh74&si=YKqZuWdEJlT6XagI&drli=1",
    )
    .unwrap();
    assert_eq!(t06.platform, PlatformKind::YouTubeMusic);
    assert_eq!(t06.initial_archetype, MediaArchetype::AudioPlaylist);

    // T-07: Spotify Track
    let t07 = UrlParser::parse_and_normalize(
        "https://open.spotify.com/track/7MtVPRGtZl6rPjMfLoI3Lh?si=c0ea33c09cfc4bd9",
    )
    .unwrap();
    assert_eq!(t07.platform, PlatformKind::Spotify);
    assert_eq!(t07.initial_archetype, MediaArchetype::SingleAudio);
    assert_eq!(
        t07.extracted_id_hint.as_deref(),
        Some("7MtVPRGtZl6rPjMfLoI3Lh")
    );

    // Multi-platform: Pinterest, Instagram, X/Twitter, Reddit GIF
    let pin = UrlParser::parse_and_normalize("https://www.pinterest.com/pin/123456789/").unwrap();
    assert_eq!(pin.platform, PlatformKind::Pinterest);
    assert_eq!(pin.initial_archetype, MediaArchetype::StaticImage);

    let gif = UrlParser::parse_and_normalize("https://i.redd.it/sample_loop.gif").unwrap();
    assert_eq!(gif.platform, PlatformKind::Reddit);
    assert_eq!(gif.initial_archetype, MediaArchetype::AnimatedGif);
}
