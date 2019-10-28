use super::*;

#[post("/users", format = "application/json", data = "<u>")]
pub fn post_user(db: sqlite::Connections, u: Json<usecases::NewUser>) -> Result<()> {
    let new_user = u.into_inner();
    let user = {
        let db = db.exclusive()?;
        usecases::create_new_user(&*db, new_user.clone())?;
        db.get_user_by_email(&new_user.email)?
    };

    notify::user_registered_kvm(&user);

    Ok(Json(()))
}

#[post(
    "/users/reset-password-request",
    format = "application/json",
    data = "<data>"
)]
pub fn post_request_password_reset(
    connections: sqlite::Connections,
    data: Json<json::RequestPasswordReset>,
) -> Result<()> {
    let req = data.into_inner();

    flows::reset_password_request(&connections, &req.email)?;

    Ok(Json(()))
}

#[post("/users/reset-password", format = "application/json", data = "<data>")]
pub fn post_reset_password(
    connections: sqlite::Connections,
    data: Json<json::ResetPassword>,
) -> Result<()> {
    let req = data.into_inner();

    let email_nonce = EmailNonce::decode_from_str(&req.token)?;
    let new_password = req.new_password.parse::<Password>()?;
    flows::reset_password_with_email_nonce(&connections, email_nonce, new_password)?;

    Ok(Json(()))
}

#[delete("/users/<email>")]
pub fn delete_user(db: sqlite::Connections, user: Login, email: String) -> Result<()> {
    usecases::delete_user(&*db.exclusive()?, &user.0, &email)?;
    Ok(Json(()))
}

#[get("/users/<email>", format = "application/json")]
pub fn get_user(db: sqlite::Connections, user: Login, email: String) -> Result<json::User> {
    let user = usecases::get_user(&*db.shared()?, &user.0, &email)?;
    Ok(Json(user.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ports::web::{api::tests::prelude::*, tests::register_user};

    #[test]
    fn reset_password() {
        let (client, db) = setup();
        register_user(&db, "user@example.com", "secret", true);

        // User sends the request
        let res = client
            .post("/users/reset-password-request")
            .header(ContentType::JSON)
            .body(r#"{"email":"user@example.com"}"#)
            .dispatch();
        assert_eq!(res.status(), Status::Ok);

        // User gets an email with the corresponding token
        let token = db
            .shared()
            .unwrap()
            .get_user_token_by_email("user@example.com")
            .unwrap()
            .email_nonce
            .encode_to_string();
        assert_eq!(
            "user@example.com",
            EmailNonce::decode_from_str(&token).unwrap().email
        );

        // User send the new password to the server
        let res = client
            .post("/users/reset-password")
            .header(ContentType::JSON)
            .body(format!(
                "{{\"token\":\"{}\",\"new_password\":\"12345678\"}}",
                token
            ))
            .dispatch();
        assert_eq!(res.status(), Status::Ok);

        // User can't login with old password
        let res = client
            .post("/login")
            .header(ContentType::JSON)
            .body(r#"{"email":"user@example.com","password":"secret"}"#)
            .dispatch();
        assert_eq!(res.status(), Status::Unauthorized);

        // User can login with the new password
        let res = client
            .post("/login")
            .header(ContentType::JSON)
            .body(r#"{"email":"user@example.com","password":"12345678"}"#)
            .dispatch();
        assert_eq!(res.status(), Status::Ok);
    }
}
