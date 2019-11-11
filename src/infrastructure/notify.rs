#[cfg(feature = "email")]
use super::mail;

use crate::{adapters::user_communication, core::prelude::*};

#[cfg(all(not(test), feature = "email"))]
fn send_email(mail: String) {
    std::thread::spawn(move || {
        if let Err(err) = mail::sendmail::send(&mail) {
            warn!("Could not send e-mail: {}", err);
        }
    });
}

/// Don't actually send emails while running the tests or
/// if the `email` feature is disabled.
#[cfg(all(test, feature = "email"))]
fn send_email(email: String) {
    debug!("Would send e-mail: {}", email);
}

#[cfg(feature = "email")]
pub fn compose_and_send_email(to: &str, subject: &str, body: &str) {
    match mail::compose(&[to], subject, body) {
        Ok(email) => send_email(email),
        Err(err) => {
            warn!("Failed to compose e-mail: {}", err);
        }
    }
}

#[cfg(feature = "email")]
pub fn compose_and_send_emails(recipients: &[String], subject: &str, body: &str) {
    debug!("Sending e-mails to: {:?}", recipients);
    for to in recipients {
        compose_and_send_email(to, subject, body);
    }
}

pub fn entry_added(email_addresses: &[String], entry: &PlaceRev, all_categories: Vec<Category>) {
    let (_, categories) = Category::split_from_tags(entry.tags.clone());
    let category_names: Vec<String> = all_categories
        .into_iter()
        .filter(|c1| categories.iter().any(|c2| c1.uid == c2.uid))
        .map(|c| c.name())
        .collect();
    let content = user_communication::entry_added_email(entry, &category_names);

    #[cfg(feature = "email")]
    {
        info!(
            "Sending e-mails to {} recipients after new entry {} added",
            email_addresses.len(),
            entry.uid
        );
        compose_and_send_emails(email_addresses, &content.subject, &content.body);
    }
}

pub fn entry_updated(email_addresses: &[String], entry: &PlaceRev, all_categories: Vec<Category>) {
    let (_, categories) = Category::split_from_tags(entry.tags.clone());
    let category_names: Vec<String> = all_categories
        .into_iter()
        .filter(|c1| categories.iter().any(|c2| c1.uid == c2.uid))
        .map(|c| c.name())
        .collect();
    let content = user_communication::entry_changed_email(entry, &category_names);

    #[cfg(feature = "email")]
    {
        info!(
            "Sending e-mails to {} recipients after entry {} updated",
            email_addresses.len(),
            entry.uid
        );
        compose_and_send_emails(email_addresses, &content.subject, &content.body);
    }
}

pub fn user_registered_kvm(user: &User) {
    let token = EmailNonce {
        email: user.email.clone(),
        nonce: Nonce::new(),
    }
    .encode_to_string();
    let url = format!("https://kartevonmorgen.org/#/?confirm_email={}", token);
    user_registered(user, &url);
}

pub fn user_registered_ofdb(user: &User) {
    let token = EmailNonce {
        email: user.email.clone(),
        nonce: Nonce::new(),
    }
    .encode_to_string();
    let url = format!("https://openfairdb.org/register/confirm/{}", token);
    user_registered(user, &url);
}

pub fn user_registered(user: &User, url: &str) {
    let content = user_communication::user_registration_email(&url);

    #[cfg(feature = "email")]
    {
        info!("Sending confirmation e-mail to user {}", user.email);
        compose_and_send_email(&user.email, &content.subject, &content.body);
    }
}

pub fn user_reset_password_requested(email_nonce: &EmailNonce) {
    let url = format!(
        "https://openfairdb.org/reset-password?token={}",
        email_nonce.encode_to_string()
    );
    let content = user_communication::user_reset_password_email(&url);

    #[cfg(feature = "email")]
    {
        info!(
            "Sending e-mail to {} after password reset requested",
            email_nonce.email
        );
        compose_and_send_emails(
            &[email_nonce.email.to_owned()],
            &content.subject,
            &content.body,
        );
    }
}
