//! Exit-code and error-code stability tests. These codes are part of the
//! BossHogg public contract (docs/conventions.md) — any change here is a
//! SemVer-major bump.

use bosshogg::BosshoggError;

#[test]
fn exit_codes_match_conventions() {
    assert_eq!(BosshoggError::MissingApiKey.exit_code(), 10);
    assert_eq!(BosshoggError::InvalidApiKey.exit_code(), 11);
    assert_eq!(
        BosshoggError::MissingScope {
            scope: "query:read".into(),
            message: "required".into()
        }
        .exit_code(),
        12,
    );
    assert_eq!(BosshoggError::NotFound("foo".into()).exit_code(), 20);
    assert_eq!(BosshoggError::BadRequest("bad".into()).exit_code(), 30);
    assert_eq!(
        BosshoggError::RateLimit {
            retry_after_s: 5,
            bucket: "query".into()
        }
        .exit_code(),
        40,
    );
    assert_eq!(
        BosshoggError::ServerError {
            status: 503,
            message: "upstream".into()
        }
        .exit_code(),
        50,
    );
    assert_eq!(BosshoggError::HogQL("syntax".into()).exit_code(), 30);
    assert_eq!(BosshoggError::Config("missing".into()).exit_code(), 71);
}

#[test]
fn error_codes_are_screaming_snake_case_and_stable() {
    assert_eq!(BosshoggError::MissingApiKey.error_code(), "AUTH_MISSING");
    assert_eq!(BosshoggError::InvalidApiKey.error_code(), "AUTH_INVALID");
    assert_eq!(
        BosshoggError::MissingScope {
            scope: "x".into(),
            message: "y".into(),
        }
        .error_code(),
        "AUTH_SCOPE",
    );
    assert_eq!(
        BosshoggError::NotFound("n".into()).error_code(),
        "NOT_FOUND"
    );
    assert_eq!(
        BosshoggError::BadRequest("b".into()).error_code(),
        "BAD_REQUEST"
    );
    assert_eq!(
        BosshoggError::RateLimit {
            retry_after_s: 1,
            bucket: "query".into()
        }
        .error_code(),
        "RATE_LIMITED",
    );
    assert_eq!(
        BosshoggError::ServerError {
            status: 500,
            message: "m".into(),
        }
        .error_code(),
        "UPSTREAM",
    );
    assert_eq!(BosshoggError::HogQL("e".into()).error_code(), "BAD_REQUEST");
    assert_eq!(BosshoggError::Config("c".into()).error_code(), "CONFIG");
}
