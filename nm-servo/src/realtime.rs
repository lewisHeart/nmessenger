use crate::{
    activities,
    auth::Session,
    authz,
    error::{Error, Result},
    state::AppState,
    validation,
};
use base64::{Engine, engine::general_purpose::STANDARD};
use nm_protocol::*;
use sqlx::PgConnection;
use uuid::Uuid;

pub fn validate_media(stream: MediaStream, encoded: &str) -> Result<()> {
    if encoded.starts_with(ENCRYPTED_PREFIX) {
        crate::messages::validate_envelope(encoded, 480 * 1024)?;
        return Ok(());
    }
    if encoded.len() > MAX_MEDIA_FRAME_BYTES.div_ceil(3) * 4 {
        return Err(Error::too_large());
    }
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| Error::bad("Base64 inválido"))?;
    if bytes.is_empty() || bytes.len() > MAX_MEDIA_FRAME_BYTES {
        return Err(Error::too_large());
    }
    match stream {
        MediaStream::Audio => {
            // At most one second of PCM mono 16 kHz, little-endian signed i16.
            if bytes.len() % 2 != 0 || bytes.len() > 32_000 {
                return Err(Error::bad("Bloco PCM inválido"));
            }
        }
        MediaStream::Camera | MediaStream::Screen => {
            if bytes.len() < 4
                || !bytes.starts_with(&[0xff, 0xd8, 0xff])
                || !bytes.ends_with(&[0xff, 0xd9])
            {
                return Err(Error::bad("Quadro deve ser JPEG"));
            }
        }
    }
    Ok(())
}

/// The wire variants are shared DTOs; identity always comes from the session.
pub async fn handle(
    state: &AppState,
    session: &Session,
    connection: Uuid,
    event: ClientEvent,
) -> Result<Option<ServerEvent>> {
    crate::auth::valid(state, session).await?;
    match event {
        ClientEvent::Authenticate { .. } => Err(Error::bad("WebSocket já autenticado")),
        ClientEvent::Ping => {
            state
                .rate(&format!("ws-ping:{}", session.user_id), 30, 10)
                .await?;
            Ok(Some(ServerEvent::Pong))
        }
        ClientEvent::Typing {
            conversation_id,
            is_typing,
        } => {
            state
                .rate(&format!("typing:{}", session.user_id), 20, 10)
                .await?;
            let mut tx = state.db.begin().await?;
            authz::shared(&mut tx).await?;
            let members = authz::member(&mut tx, session.user_id, conversation_id).await?;
            if !crate::privacy::shares(&mut tx, session.user_id, "typing_indicators").await? {
                tx.commit().await?;
                return Ok(None);
            }
            authz::peers(
                &mut tx,
                session.user_id,
                &members
                    .iter()
                    .copied()
                    .filter(|u| *u != session.user_id)
                    .collect::<Vec<_>>(),
            )
            .await?;
            state
                .publish(
                    members
                        .into_iter()
                        .filter(|u| *u != session.user_id)
                        .collect(),
                    ServerEvent::Typing {
                        conversation_id,
                        user_id: session.user_id,
                        is_typing,
                    },
                )
                .await;
            tx.commit().await?;
            Ok(None)
        }
        ClientEvent::Signal { activity_id, data } => {
            let encrypted = if let Some(envelope) = data.get("e2ee") {
                let encoded = format!(
                    "{ENCRYPTED_PREFIX}{}",
                    serde_json::to_string(envelope).map_err(|_| Error::bad("Sinal inválido"))?
                );
                Some(crate::messages::validate_envelope(&encoded, 96 * 1024)?)
            } else {
                None
            };
            validation::json_size(
                &data,
                if encrypted.is_some() {
                    96 * 1024
                } else {
                    16 * 1024
                },
            )?;
            if !data.is_object() {
                return Err(Error::bad("Sinal deve ser objeto JSON"));
            }
            state
                .rate(
                    &format!("signal:{}:{activity_id}", session.user_id),
                    120,
                    10,
                )
                .await?;
            let mut tx = state.db.begin().await?;
            authz::shared(&mut tx).await?;
            let (activity, recipients) =
                activities::active(&mut tx, session.user_id, activity_id).await?;
            if let Some(envelope) = encrypted {
                crate::encryption::route(
                    &mut tx,
                    session,
                    activity.conversation_id,
                    &recipients,
                    &envelope,
                )
                .await?;
            } else {
                require_legacy(&mut tx, session.user_id).await?;
            }
            state
                .publish(
                    recipients
                        .into_iter()
                        .filter(|u| *u != session.user_id)
                        .collect(),
                    ServerEvent::Signal {
                        activity_id,
                        from_user_id: session.user_id,
                        data,
                    },
                )
                .await;
            tx.commit().await?;
            Ok(None)
        }
        ClientEvent::Media {
            activity_id,
            stream,
            sequence,
            data,
        } => {
            validate_media(stream, &data)?;
            let maximum = match stream {
                MediaStream::Audio => 60,
                MediaStream::Camera => 15,
                MediaStream::Screen => 10,
            };
            let stream_key = validation::wire(stream)?;
            state
                .rate(
                    &format!("media:{}:{activity_id}:{stream_key}", session.user_id),
                    maximum,
                    1,
                )
                .await?;
            state
                .rate(&format!("media-total:{}", session.user_id), 100, 1)
                .await?;
            let mut tx = state.db.begin().await?;
            authz::shared(&mut tx).await?;
            let (activity, recipients) =
                activities::active(&mut tx, session.user_id, activity_id).await?;
            if data.starts_with(ENCRYPTED_PREFIX) {
                let envelope = crate::messages::validate_envelope(&data, 480 * 1024)?;
                crate::encryption::route(
                    &mut tx,
                    session,
                    activity.conversation_id,
                    &recipients,
                    &envelope,
                )
                .await?;
            } else {
                require_legacy(&mut tx, session.user_id).await?;
            }
            let allowed = matches!(
                (activity.kind, stream),
                (ActivityKind::AudioCall, MediaStream::Audio | MediaStream::Camera | MediaStream::Screen)
                    | (
                        ActivityKind::VideoCall,
                        MediaStream::Audio | MediaStream::Camera | MediaStream::Screen
                    )
                    | (
                        ActivityKind::RemoteAssistance,
                        MediaStream::Audio | MediaStream::Screen
                    )
            );
            if !allowed {
                return Err(Error::forbidden());
            }
            // Drop duplicate/out-of-order chunks per sending device and stream.
            let fresh: i64 = redis::Script::new(
                "local n=redis.call('GET',KEYS[1]);
                if n and (string.len(n)>string.len(ARGV[1]) or
                    (string.len(n)==string.len(ARGV[1]) and n>=ARGV[1])) then return 0 end;
                redis.call('SET',KEYS[1],ARGV[1],'EX',120); return 1",
            )
            .key(format!(
                "nm:sequence:{connection}:{activity_id}:{stream_key}"
            ))
            .arg(sequence.to_string())
            .invoke_async(&mut state.redis.clone())
            .await?;
            if fresh == 1 {
                state
                    .publish(
                        recipients
                            .into_iter()
                            .filter(|u| *u != session.user_id)
                            .collect(),
                        ServerEvent::Media {
                            activity_id,
                            from_user_id: session.user_id,
                            stream,
                            sequence,
                            data,
                        },
                    )
                    .await;
            }
            tx.commit().await?;
            Ok(None)
        }
    }
}

async fn require_legacy(db: &mut PgConnection, user: Uuid) -> Result<()> {
    let enrolled: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM encryption_keys WHERE user_id=$1)")
            .bind(user)
            .fetch_one(db)
            .await?;
    if enrolled {
        return Err(Error::bad(
            "Este dispositivo exige mídia e sinais criptografados",
        ));
    }
    Ok(())
}

async fn visible_contact(db: &mut PgConnection, viewer: Uuid, user: Uuid) -> Result<bool> {
    if viewer == user {
        return Ok(true);
    }
    if authz::blocked(db, viewer, user).await? {
        return Ok(false);
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM contacts WHERE state='accepted' AND
        ((owner_id=$1 AND user_id=$2) OR (owner_id=$2 AND user_id=$1))",
    )
    .bind(viewer)
    .bind(user)
    .fetch_one(db)
    .await?;
    Ok(count == 2)
}

fn permitted<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(error) if matches!(error.status.as_u16(), 403 | 404) => Ok(None),
        Err(error) => Err(error),
    }
}

async fn current_public_presence(
    state: &AppState,
    db: &mut PgConnection,
    user: Uuid,
    proposed: Presence,
) -> Result<bool> {
    if proposed == Presence::Offline {
        return Ok(true);
    }
    if proposed == Presence::Invisible {
        return Ok(false);
    }
    let current: String = sqlx::query_scalar("SELECT presence FROM users WHERE id=$1")
        .bind(user)
        .fetch_one(db)
        .await?;
    let current: Presence = validation::parse(&current)?;
    Ok(current.visible() == proposed && crate::presence::online(state, user).await?)
}

/// Re-check at delivery time: a queued event must not restore removed access.
pub async fn can_deliver(state: &AppState, viewer: Uuid, event: &ServerEvent) -> Result<bool> {
    let mut tx = state.db.begin().await?;
    authz::shared(&mut tx).await?;
    let allowed = match event {
        ServerEvent::Message { message } | ServerEvent::MessageUpdated { message } => {
            if permitted(authz::member(&mut tx, viewer, message.conversation_id).await)?.is_none() {
                false
            } else {
                sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM conversation_members cm
                    JOIN messages m ON m.id=$4 AND m.conversation_id=cm.conversation_id
                    WHERE cm.conversation_id=$1 AND cm.user_id=$2 AND cm.hidden_at<$3
                    AND m.body=$5 AND m.style=$6)",
                )
                .bind(message.conversation_id)
                .bind(viewer)
                .bind(message.created_at)
                .bind(message.id)
                .bind(&message.body)
                .bind(sqlx::types::Json(&message.style))
                .fetch_one(&mut *tx)
                .await?
            }
        }
        ServerEvent::MessageDeleted { conversation_id, .. } => {
            permitted(authz::member(&mut tx, viewer, *conversation_id).await)?.is_some()
        }
        ServerEvent::Typing {
            conversation_id,
            user_id,
            ..
        }
        | ServerEvent::Read {
            conversation_id,
            user_id,
            ..
        } => match permitted(authz::member(&mut tx, viewer, *conversation_id).await)? {
            Some(members) => {
                let setting = if matches!(event, ServerEvent::Typing { .. }) {
                    "typing_indicators"
                } else {
                    "read_receipts"
                };
                members.contains(user_id)
                    && (viewer == *user_id
                        || crate::privacy::shares(&mut tx, *user_id, setting).await?)
            }
            None => false,
        },
        ServerEvent::Activity { activity } => {
            if permitted(authz::member(&mut tx, viewer, activity.conversation_id).await)?.is_none()
            {
                false
            } else {
                sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM activity_participants WHERE activity_id=$1 AND user_id=$2)")
                    .bind(activity.id).bind(viewer).fetch_one(&mut *tx).await?
            }
        }
        ServerEvent::Signal {
            activity_id,
            from_user_id,
            ..
        }
        | ServerEvent::Media {
            activity_id,
            from_user_id,
            ..
        } => match permitted(activities::active(&mut tx, viewer, *activity_id).await)? {
            Some((_, members)) => members.contains(from_user_id),
            None => false,
        },
        ServerEvent::Presence { user_id, presence } => {
            if *presence == Presence::Offline {
                true
            } else {
                visible_contact(&mut tx, viewer, *user_id).await?
                    && current_public_presence(state, &mut tx, *user_id, *presence).await?
            }
        }
        ServerEvent::ProfileUpdated { profile } => {
            profile.id == viewer
                || (visible_contact(&mut tx, viewer, profile.id).await?
                    && current_public_presence(state, &mut tx, profile.id, profile.presence)
                        .await?)
        }
        _ => true,
    };
    tx.commit().await?;
    Ok(allowed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn media_frames_are_bounded_and_typed() {
        assert!(validate_media(MediaStream::Audio, &STANDARD.encode([0u8; 640])).is_ok());
        assert!(validate_media(MediaStream::Audio, &STANDARD.encode([0u8; 3])).is_err());
        assert!(validate_media(MediaStream::Audio, &STANDARD.encode([0u8; 32002])).is_err());
        assert!(validate_media(MediaStream::Camera, &STANDARD.encode([0u8; 640])).is_err());
        assert!(
            validate_media(
                MediaStream::Camera,
                &STANDARD.encode([255, 216, 255, 0, 255, 217])
            )
            .is_ok()
        );
        assert!(validate_media(MediaStream::Audio, "%%").is_err());
    }
}
