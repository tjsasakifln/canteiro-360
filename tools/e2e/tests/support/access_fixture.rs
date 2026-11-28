//! Shared authenticated test fixtures for permissions and contract history.
use super::{Harness, TestResult};
use base64::Engine;
use canteiro_contracts::{access::*, auth::AuthCommand, organizations::*};
use hmac::{Hmac, Mac};
use sha1::Sha1;
use std::time::{SystemTime, UNIX_EPOCH};

// A caixa de testes é compartilhada pelos cenários do mesmo executável.
static ACCOUNT_MAIL: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub(crate) struct AccountFixture {
    pub email: String,
    pub session: String,
}

fn totp(secret: &str) -> TestResult<String> {
    let bytes = data_encoding::BASE32_NOPAD.decode(secret.as_bytes())?;
    let counter = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() / 30;
    let mut mac = <Hmac<Sha1> as Mac>::new_from_slice(&bytes)?;
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    let offset = usize::from(digest[19] & 15);
    let value = u32::from_be_bytes(digest[offset..offset + 4].try_into()?) & 0x7fff_ffff;
    Ok(format!("{:06}", value % 1_000_000))
}

pub(crate) async fn account(h: &Harness, label: &str, mfa: bool) -> TestResult<AccountFixture> {
    let mut random = [0u8; 32];
    getrandom::fill(&mut random).map_err(|_| "Unable to generate fixture credentials")?;
    let nonce = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(random);
    let email = format!(
        "permissions-{label}-{}@example.invalid",
        nonce[..32].to_ascii_lowercase()
    );
    let password = format!("Acesso isolado {nonce}");
    let mail_guard = ACCOUNT_MAIL.lock().await;
    let mut result = h.auth.context(None).await?;
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::Register {
                email: email.clone(),
                password: password.clone(),
            },
            &email,
        )
        .await?;
    let mut code = None;
    for message in h.auth.pending_mail(25).await? {
        if message.email == email && message.kind == "verify_email" {
            code = Some(message.token.clone());
            h.auth.mark_mail_delivered(&message.id, "fixture").await?;
        }
    }
    drop(mail_guard);
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::VerifyEmail {
                token: code.ok_or("Missing verification fixture")?,
                password: password.clone(),
            },
            &email,
        )
        .await?;
    result = h
        .auth
        .execute(
            &result.session_token,
            AuthCommand::Login {
                email: email.clone(),
                password: password.clone(),
            },
            &email,
        )
        .await?;
    if mfa {
        result = h
            .auth
            .execute(
                &result.session_token,
                AuthCommand::StartMfa { password },
                &email,
            )
            .await?;
        let setup = result
            .mfa_setup
            .as_ref()
            .ok_or("Missing MFA enrollment fixture")?;
        let code = totp(setup)?;
        result = h
            .auth
            .execute(
                &result.session_token,
                AuthCommand::ConfirmMfa { code },
                &email,
            )
            .await?;
    }
    Ok(AccountFixture {
        email,
        session: result.session_token,
    })
}

pub(crate) fn scope(state: &AccessState) -> OrganizationScope {
    let org = state
        .organization
        .as_ref()
        .expect("selected fixture company");
    OrganizationScope {
        organization_id: org.id.clone(),
        organization_version: org.version,
        context_version: state.context_version,
    }
}
pub(crate) async fn company(h: &Harness, owner: &AccountFixture, name: &str) -> TestResult<String> {
    let r = h
        .auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Create { name: name.into() },
        )
        .await?;
    let id = r
        .state
        .organizations
        .into_iter()
        .find(|o| o.name == name)
        .ok_or("fixture company missing")?
        .id;
    h.auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Select {
                organization_id: id.clone(),
                expected_context_version: r.state.context_version,
            },
        )
        .await?;
    Ok(id)
}
pub(crate) async fn join(
    h: &Harness,
    owner: &AccountFixture,
    person: &AccountFixture,
    id: &str,
) -> TestResult {
    let state = h.auth.organizations_context(&owner.session).await?.state;
    let org = state
        .active
        .ok_or("fixture company not selected")?
        .organization;
    let invitation = h
        .auth
        .organizations_execute(
            &owner.session,
            OrganizationCommand::Invite {
                scope: OrganizationScope {
                    organization_id: org.id,
                    organization_version: org.version,
                    context_version: state.context_version,
                },
                email: person.email.clone(),
                role: OrganizationRole::Member,
                valid_for_hours: 24,
                membership_valid_until: None,
            },
        )
        .await?;
    let response = h
        .auth
        .organizations_execute(
            &person.session,
            OrganizationCommand::AcceptInvitation {
                token: invitation
                    .invitation_code
                    .ok_or("fixture invitation missing")?,
            },
        )
        .await?;
    h.auth
        .organizations_execute(
            &person.session,
            OrganizationCommand::Select {
                organization_id: id.into(),
                expected_context_version: response.state.context_version,
            },
        )
        .await?;
    Ok(())
}
pub(crate) async fn fixture_grant(
    h: &Harness,
    owner: &AccountFixture,
    person: &AccountFixture,
    scope_id: &str,
    role: AccessRole,
    actions: Vec<AccessAction>,
) -> TestResult {
    let r = h.auth.access_context(&owner.session).await?;
    h.auth
        .access_execute(
            &owner.session,
            AccessCommand::Grant {
                context: scope(&r.state),
                scope_id: scope_id.into(),
                user_email: person.email.clone(),
                role,
                actions,
                max_amount: "1000.00".parse()?,
                valid_from: "2020-01-01T00:00:00Z".into(),
                valid_until: Some("2090-01-01T00:00:00Z".into()),
                required: false,
            },
        )
        .await?;
    Ok(())
}
