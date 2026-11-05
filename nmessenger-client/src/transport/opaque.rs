use super::{Api, api::{bounded_json, checked}};
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::{AuthResponse, secure_auth::{CipherSuite, opaque_ke}};
use opaque_ke::{ClientLogin, ClientLoginFinishParameters, ClientRegistration, ClientRegistrationFinishParameters, CredentialResponse, RegistrationResponse};
use serde::Deserialize;
use zeroize::Zeroizing;

#[derive(Deserialize)]
struct Challenge { challenge: uuid::Uuid, response: String }
#[derive(Deserialize)]
struct Registration { response: String }

pub async fn login(client: &reqwest::Client, base: &str, email: &str, password: &str, device: &str) -> Result<Option<(AuthResponse, Zeroizing<Vec<u8>>)>, String> {
    let password = Zeroizing::new(password.to_owned());
    let start = ClientLogin::<CipherSuite>::start(&mut opaque_ke::rand::rngs::OsRng, password.as_bytes()).map_err(|_| "Não foi possível iniciar a autenticação segura")?;
    let response = client.post(format!("{base}/auth/opaque/login/start"))
        .json(&serde_json::json!({"email":email,"device_name":device,"request":STANDARD.encode(start.message.serialize())}))
        .send().await.map_err(|e| e.to_string())?;
    if matches!(response.status(), reqwest::StatusCode::NOT_FOUND | reqwest::StatusCode::CONFLICT) { return Ok(None); }
    let challenge: Challenge = bounded_json(checked(response).await?).await?;
    let encoded = STANDARD.decode(challenge.response).map_err(|_| "Resposta segura inválida")?;
    let response = CredentialResponse::<CipherSuite>::deserialize(&encoded).map_err(|_| "Resposta segura inválida")?;
    let finish = tokio::task::spawn_blocking(move || start.state.finish(password.as_bytes(), response, ClientLoginFinishParameters::default()))
        .await.map_err(|e| e.to_string())?.map_err(|_| "E-mail ou senha incorretos.")?;
    let export = Zeroizing::new(finish.export_key.to_vec());
    let response = checked(client.post(format!("{base}/auth/opaque/login/finish"))
        .json(&serde_json::json!({"challenge":challenge.challenge,"finalization":STANDARD.encode(finish.message.serialize())}))
        .send().await.map_err(|e| e.to_string())?).await?;
    Ok(Some((bounded_json(response).await?, export)))
}

pub async fn enroll(api: &Api, password: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    let password = Zeroizing::new(password.to_owned());
    let start = ClientRegistration::<CipherSuite>::start(&mut opaque_ke::rand::rngs::OsRng, password.as_bytes())
        .map_err(|_| "Não foi possível iniciar a proteção do histórico")?;
    let response = api.request(reqwest::Method::POST, "/auth/opaque/register/start")
        .json(&serde_json::json!({"request":STANDARD.encode(start.message.serialize())}))
        .send().await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    let response: Registration = bounded_json(checked(response).await?).await?;
    let response = RegistrationResponse::<CipherSuite>::deserialize(&STANDARD.decode(response.response).map_err(|_| "Resposta segura inválida")?)
        .map_err(|_| "Resposta segura inválida")?;
    let finish = tokio::task::spawn_blocking(move || start.state.finish(&mut opaque_ke::rand::rngs::OsRng, password.as_bytes(), response, ClientRegistrationFinishParameters::default()))
        .await.map_err(|e| e.to_string())?.map_err(|_| "Não foi possível proteger o histórico")?;
    let export = Zeroizing::new(finish.export_key.to_vec());
    checked(api.request(reqwest::Method::POST, "/auth/opaque/register/finish")
        .json(&serde_json::json!({"record":STANDARD.encode(finish.message.serialize())}))
        .send().await.map_err(|e| e.to_string())?).await?;
    Ok(Some(export))
}

pub async fn recover(api: &Api, password: &str) -> Result<Option<Zeroizing<Vec<u8>>>, String> {
    let password = Zeroizing::new(password.to_owned());
    let start = ClientLogin::<CipherSuite>::start(&mut opaque_ke::rand::rngs::OsRng, password.as_bytes())
        .map_err(|_| "Não foi possível iniciar a recuperação")?;
    let response = api.request(reqwest::Method::POST, "/auth/opaque/recovery/start")
        .json(&serde_json::json!({"request":STANDARD.encode(start.message.serialize())}))
        .send().await.map_err(|e| e.to_string())?;
    if response.status() == reqwest::StatusCode::NOT_FOUND { return Ok(None); }
    let response: Registration = bounded_json(checked(response).await?).await?;
    let response = CredentialResponse::<CipherSuite>::deserialize(&STANDARD.decode(response.response).map_err(|_| "Resposta segura inválida")?)
        .map_err(|_| "Resposta segura inválida")?;
    let finish = tokio::task::spawn_blocking(move || start.state.finish(password.as_bytes(), response, ClientLoginFinishParameters::default()))
        .await.map_err(|e| e.to_string())?.map_err(|_| "Senha anterior incorreta para recuperar o histórico")?;
    Ok(Some(Zeroizing::new(finish.export_key.to_vec())))
}
