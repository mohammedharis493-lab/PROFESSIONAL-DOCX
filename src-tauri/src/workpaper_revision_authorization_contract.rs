//! Proposed native authoring preflight. NOT WIRED to any Tauri command.
//! No production verified-session constructor, grant issuer, grant schema,
//! Save method, evidence copier or sign-off authority exists here.
//! A positive preflight is NEVER sufficient to authorize a write:
//! the future native command must recheck identity, grants, latest revision,
//! and exact evidence under its final SQLite IMMEDIATE transaction.

use uuid::Uuid;

/// Proposed workpaper-wide scopes. Neither scope is the existing Normal Data
/// MODIFY_WORKPAPER_REVISION grant, which targets one exact immutable revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum WorkpaperAuthoringPermission {
    ReadWorkpaper,
    ReviseWorkpaper,
}

/// Fields are private; no production constructor exists. Never derive this
/// credential from user-submitted IDs, role strings, or frontend state.
pub(crate) struct VerifiedNativeWorkpaperSession {
    subject_id: Uuid,
    session_id: Uuid,
    expires_at_ms: i64,
}

/// Future adapter contract only. A real adapter must recheck live session,
/// grants, workpaper/engagement scope and revocation transactionally.
pub(crate) trait WorkpaperAuthoringPolicy {
    fn session_is_active(&self, subject_id: Uuid, session_id: Uuid, now_ms: i64) -> bool;
    fn has_workpaper_grant(
        &self,
        subject_id: Uuid,
        permission: WorkpaperAuthoringPermission,
        workpaper_id: Uuid,
        now_ms: i64,
    ) -> bool;
}

/// The only shipping adapter. Fails closed regardless of local SQLite rows.
pub(crate) struct DenyAllWorkpaperAuthoringPolicy;

impl WorkpaperAuthoringPolicy for DenyAllWorkpaperAuthoringPolicy {
    fn session_is_active(&self, _subject_id: Uuid, _session_id: Uuid, _now_ms: i64) -> bool {
        false
    }

    fn has_workpaper_grant(
        &self,
        _subject_id: Uuid,
        _permission: WorkpaperAuthoringPermission,
        _workpaper_id: Uuid,
        _now_ms: i64,
    ) -> bool {
        false
    }
}

/// Opaque denial must not disclose another user, grant or resource existence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct WorkpaperAuthoringDenied;

/// Pure, non-authorizing preflight. A future native time provider supplies
/// now_ms, NEVER the caller. This is not an authorized Save token.
pub(crate) fn check_workpaper_revision_authoring_preflight(
    session: Option<&VerifiedNativeWorkpaperSession>,
    requested_workpaper_id: &str,
    policy: &dyn WorkpaperAuthoringPolicy,
    now_ms: i64,
) -> Result<(), WorkpaperAuthoringDenied> {
    let session = session.ok_or(WorkpaperAuthoringDenied)?;
    let workpaper =
        Uuid::parse_str(requested_workpaper_id).map_err(|_| WorkpaperAuthoringDenied)?;
    if now_ms < 0
        || session.subject_id.is_nil()
        || session.session_id.is_nil()
        || workpaper.is_nil()
        || session.expires_at_ms <= now_ms
        || !policy.session_is_active(session.subject_id, session.session_id, now_ms)
    {
        return Err(WorkpaperAuthoringDenied);
    }
    for permission in [
        WorkpaperAuthoringPermission::ReadWorkpaper,
        WorkpaperAuthoringPermission::ReviseWorkpaper,
    ] {
        if !policy.has_workpaper_grant(session.subject_id, permission, workpaper, now_ms) {
            return Err(WorkpaperAuthoringDenied);
        }
    }
    Ok(())
}

#[cfg(test)]
mod non_authorizing_contract_tests {
    use super::*;
    use std::collections::HashSet;

    /// Only fixture-created in cfg(test); never compiled into installed app.
    fn fixture_session(subject_id: Uuid, expires_at_ms: i64) -> VerifiedNativeWorkpaperSession {
        VerifiedNativeWorkpaperSession {
            subject_id,
            session_id: Uuid::new_v4(),
            expires_at_ms,
        }
    }

    #[derive(Default)]
    struct FixturePolicy {
        live_session: bool,
        grants: HashSet<(Uuid, WorkpaperAuthoringPermission, Uuid)>,
    }

    impl FixturePolicy {
        fn grant(&mut self, person: Uuid, right: WorkpaperAuthoringPermission, target: Uuid) {
            self.grants.insert((person, right, target));
        }
    }

    impl WorkpaperAuthoringPolicy for FixturePolicy {
        fn session_is_active(&self, _person: Uuid, _session: Uuid, _now_ms: i64) -> bool {
            self.live_session
        }

        fn has_workpaper_grant(
            &self,
            person: Uuid,
            right: WorkpaperAuthoringPermission,
            target: Uuid,
            _now_ms: i64,
        ) -> bool {
            self.grants.contains(&(person, right, target))
        }
    }

    #[test]
    fn no_native_session_denies_even_hypothetical_fixture_grants() {
        let target = Uuid::new_v4();
        let person = Uuid::new_v4();
        let mut mock = FixturePolicy {
            live_session: true,
            ..FixturePolicy::default()
        };
        mock.grant(person, WorkpaperAuthoringPermission::ReadWorkpaper, target);
        mock.grant(person, WorkpaperAuthoringPermission::ReviseWorkpaper, target);
        assert_eq!(
            check_workpaper_revision_authoring_preflight(None, &target.to_string(), &mock, 100),
            Err(WorkpaperAuthoringDenied)
        );
    }

    #[test]
    fn only_shipping_deny_all_policy_refuses_fixture_verified_session() {
        let session = fixture_session(Uuid::new_v4(), 500);
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &Uuid::new_v4().to_string(),
                &DenyAllWorkpaperAuthoringPolicy,
                100,
            ),
            Err(WorkpaperAuthoringDenied)
        );
    }

    #[test]
    fn distinct_read_and_revise_grants_must_both_match_exact_workpaper() {
        let person = Uuid::new_v4();
        let session = fixture_session(person, 500);
        let target = Uuid::new_v4();
        let foreign = Uuid::new_v4();
        let mut mock = FixturePolicy {
            live_session: true,
            ..FixturePolicy::default()
        };
        mock.grant(person, WorkpaperAuthoringPermission::ReadWorkpaper, target);
        mock.grant(person, WorkpaperAuthoringPermission::ReviseWorkpaper, foreign);
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &target.to_string(),
                &mock,
                100,
            ),
            Err(WorkpaperAuthoringDenied)
        );
        mock.grant(person, WorkpaperAuthoringPermission::ReviseWorkpaper, target);
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &target.to_string(),
                &mock,
                100,
            ),
            Ok(())
        );
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &foreign.to_string(),
                &mock,
                100,
            ),
            Err(WorkpaperAuthoringDenied)
        );
    }

    #[test]
    fn expired_or_revoked_session_denies_despite_test_only_grants() {
        let person = Uuid::new_v4();
        let session = fixture_session(person, 200);
        let target = Uuid::new_v4();
        let mut mock = FixturePolicy {
            live_session: true,
            ..FixturePolicy::default()
        };
        mock.grant(person, WorkpaperAuthoringPermission::ReadWorkpaper, target);
        mock.grant(person, WorkpaperAuthoringPermission::ReviseWorkpaper, target);
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &target.to_string(),
                &mock,
                199,
            ),
            Ok(())
        );
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &target.to_string(),
                &mock,
                200,
            ),
            Err(WorkpaperAuthoringDenied)
        );
        mock.live_session = false;
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &target.to_string(),
                &mock,
                100,
            ),
            Err(WorkpaperAuthoringDenied)
        );
    }

    #[test]
    fn caller_claimed_roles_and_malformed_scope_never_satisfy_native_preflight() {
        let session = fixture_session(Uuid::new_v4(), 500);
        let mock = FixturePolicy {
            live_session: true,
            ..FixturePolicy::default()
        };
        for claimed in [
            "admin",
            "reviewer",
            "MODIFY_WORKPAPER_REVISION",
            "",
            "00000000-0000-0000-0000-000000000000",
        ] {
            assert_eq!(
                check_workpaper_revision_authoring_preflight(Some(&session), claimed, &mock, 100),
                Err(WorkpaperAuthoringDenied)
            );
        }
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&fixture_session(Uuid::nil(), 500)),
                &Uuid::new_v4().to_string(),
                &mock,
                100,
            ),
            Err(WorkpaperAuthoringDenied)
        );
        assert_eq!(
            check_workpaper_revision_authoring_preflight(
                Some(&session),
                &Uuid::new_v4().to_string(),
                &mock,
                -1,
            ),
            Err(WorkpaperAuthoringDenied)
        );
    }
}
