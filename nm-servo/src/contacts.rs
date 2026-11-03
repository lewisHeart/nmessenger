use crate::{
    auth::Session,
    authz,
    error::{Error, Result},
    models::ProfileRow,
    presence,
    state::AppState,
    validation,
};
use actix_web::{HttpResponse, web};
use nm_protocol::*;
use sqlx::{FromRow, PgConnection, types::Json};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

#[derive(FromRow)]
struct ContactRow {
    #[sqlx(flatten)]
    user: ProfileRow,
    nickname: Option<String>,
    favorite: bool,
    blocked: bool,
    state: String,
    details: Json<ContactDetails>,
    category_ids: Vec<Uuid>,
}

pub async fn list(state: &AppState, user: Uuid) -> Result<Vec<Contact>> {
    let rows: Vec<ContactRow> = sqlx::query_as(
        "SELECT u.*,c.nickname,c.favorite,c.state,c.details,
         EXISTS(SELECT 1 FROM user_blocks b WHERE b.owner_id=c.owner_id AND b.user_id=c.user_id) AS blocked,
        ARRAY(SELECT category_id FROM contact_categories cc WHERE cc.owner_id=c.owner_id
            AND cc.user_id=c.user_id ORDER BY category_id) AS category_ids
        FROM contacts c JOIN users u ON u.id=c.user_id WHERE c.owner_id=$1
        ORDER BY lower(COALESCE(NULLIF(c.nickname,''),u.display_name)),u.id",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;
    let profiles =
        presence::profiles(state, user, rows.iter().map(|r| r.user.clone()).collect()).await?;
    rows.into_iter()
        .zip(profiles)
        .map(|(row, user)| {
            Ok(Contact {
                user,
                nickname: row.nickname,
                favorite: row.favorite,
                blocked: row.blocked,
                state: validation::parse(&row.state)?,
                details: row.details.0,
                category_ids: row.category_ids,
            })
        })
        .collect()
}

async fn one(state: &AppState, owner: Uuid, target: Uuid) -> Result<Contact> {
    list(state, owner)
        .await?
        .into_iter()
        .find(|c| c.user.id == target)
        .ok_or_else(Error::missing)
}

pub async fn categories(state: &AppState, user: Uuid) -> Result<Vec<Category>> {
    let rows: Vec<(Uuid, String, i32)> = sqlx::query_as(
        "SELECT id,name,position FROM categories WHERE owner_id=$1 ORDER BY position,id",
    )
    .bind(user)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, position)| Category { id, name, position })
        .collect())
}

async fn add_in_tx(
    db: &mut PgConnection,
    owner: Uuid,
    target: Uuid,
    nickname: Option<&str>,
) -> Result<()> {
    if owner == target {
        return Err(Error::bad("Não é possível adicionar a si mesmo"));
    }
    let (own_block, other_block): (bool, bool) = sqlx::query_as(
        "SELECT
        EXISTS(SELECT 1 FROM user_blocks WHERE owner_id=$1 AND user_id=$2),
        EXISTS(SELECT 1 FROM user_blocks WHERE owner_id=$2 AND user_id=$1)",
    )
    .bind(owner)
    .bind(target)
    .fetch_one(&mut *db)
    .await?;
    if other_block && !own_block {
        return Err(Error::forbidden());
    }
    let existed: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM contacts WHERE owner_id=$1 AND user_id=$2)",
    )
    .bind(owner)
    .bind(target)
    .fetch_one(&mut *db)
    .await?;
    sqlx::query(
        "INSERT INTO contacts(owner_id,user_id,state,nickname,blocked)
        VALUES($1,$2,'pending_outgoing',$3,$4) ON CONFLICT(owner_id,user_id)
        DO UPDATE SET nickname=COALESCE(EXCLUDED.nickname,contacts.nickname)",
    )
    .bind(owner)
    .bind(target)
    .bind(nickname)
    .bind(own_block)
    .execute(&mut *db)
    .await?;
    if own_block {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO contacts(owner_id,user_id,state) VALUES($2,$1,'pending_incoming')
        ON CONFLICT(owner_id,user_id) DO UPDATE SET state=CASE WHEN $3::boolean THEN contacts.state ELSE 'pending_incoming' END",
    )
    .bind(owner)
    .bind(target)
    .bind(existed)
    .execute(db)
    .await?;
    Ok(())
}

async fn changed(state: &AppState, users: Vec<Uuid>) {
    state
        .publish(users.clone(), ServerEvent::ContactsChanged)
        .await;
    state
        .publish(users, ServerEvent::ConversationsChanged)
        .await;
}

async fn get(state: web::Data<AppState>, session: Session) -> Result<web::Json<Vec<Contact>>> {
    Ok(web::Json(list(&state, session.user_id).await?))
}

async fn add(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<AddContactRequest>,
) -> Result<web::Json<Contact>> {
    let email = validation::email(&body.email)?;
    if let Some(v) = &body.nickname {
        validation::text(v, "Apelido", 0, 100)?;
    }
    state
        .rate(&format!("contacts:{}", session.user_id), 30, 60)
        .await?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let target: Uuid = sqlx::query_scalar("SELECT id FROM users WHERE email=$1")
        .bind(email)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(Error::missing)?;
    add_in_tx(&mut tx, session.user_id, target, body.nickname.as_deref()).await?;
    let target_is_bot: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM bots WHERE user_id=$1)")
        .bind(target).fetch_one(&mut *tx).await?;
    if target_is_bot && !authz::blocked(&mut tx, session.user_id, target).await? {
        sqlx::query("UPDATE contacts SET state='accepted' WHERE (owner_id=$1 AND user_id=$2) OR (owner_id=$2 AND user_id=$1)")
            .bind(session.user_id).bind(target).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    changed(&state, vec![session.user_id, target]).await;
    Ok(web::Json(one(&state, session.user_id, target).await?))
}

fn validate_details(details: &ContactDetails) -> Result<()> {
    validation::json_size(details, 24_000)?;
    for s in [
        &details.full_name,
        &details.mobile,
        &details.home_email,
        &details.work_email,
        &details.birthday,
        &details.website,
        &details.company,
        &details.job_title,
    ] {
        validation::text(s, "Detalhe", 0, 1024)?;
    }
    validation::text(&details.notes, "Notas", 0, 8000)
}

async fn update(
    state: web::Data<AppState>,
    session: Session,
    target: web::Path<Uuid>,
    body: web::Json<UpdateContactRequest>,
) -> Result<web::Json<Contact>> {
    if let Some(v) = &body.nickname {
        validation::text(v, "Apelido", 0, 100)?;
    }
    if let Some(v) = &body.details {
        validate_details(v)?;
    }
    let category_ids: Option<Vec<Uuid>> = body.category_ids.as_ref().map(|ids| {
        let mut ids = ids.clone();
        ids.sort_unstable();
        ids.dedup();
        ids
    });
    if category_ids.as_ref().is_some_and(|ids| ids.len() > 100) {
        return Err(Error::bad("Muitas categorias"));
    }
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let previous: String = sqlx::query_scalar(
        "SELECT state FROM contacts WHERE owner_id=$1 AND user_id=$2 FOR UPDATE",
    )
    .bind(session.user_id)
    .bind(*target)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(Error::missing)?;
    if let Some(blocked) = body.blocked {
        if blocked {
            sqlx::query(
                "INSERT INTO user_blocks(owner_id,user_id) VALUES($1,$2) ON CONFLICT DO NOTHING",
            )
            .bind(session.user_id)
            .bind(*target)
            .execute(&mut *tx)
            .await?;
        } else {
            sqlx::query("DELETE FROM user_blocks WHERE owner_id=$1 AND user_id=$2")
                .bind(session.user_id)
                .bind(*target)
                .execute(&mut *tx)
                .await?;
        }
    }
    if let Some(true) = body.accepted {
        if previous != "pending_incoming" && previous != "accepted" {
            return Err(Error::conflict(
                "Somente o destinatário pode aceitar o pedido",
            ));
        }
        if authz::blocked(&mut tx, session.user_id, *target).await? {
            return Err(Error::forbidden());
        }
        let result = sqlx::query(
            "UPDATE contacts SET state='accepted' WHERE
            (owner_id=$1 AND user_id=$2) OR (owner_id=$2 AND user_id=$1)",
        )
        .bind(session.user_id)
        .bind(*target)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() != 2 {
            return Err(Error::conflict("Pedido não está mais disponível"));
        }
    } else if body.accepted == Some(false) {
        sqlx::query("UPDATE contacts SET state=CASE WHEN owner_id=$1 THEN 'pending_outgoing'
            ELSE 'pending_incoming' END WHERE (owner_id=$1 AND user_id=$2) OR (owner_id=$2 AND user_id=$1)")
            .bind(session.user_id).bind(*target).execute(&mut *tx).await?;
    }
    sqlx::query(
        "UPDATE contacts SET nickname=COALESCE($3,nickname),favorite=COALESCE($4,favorite),
        blocked=COALESCE($5,blocked),details=COALESCE($6,details) WHERE owner_id=$1 AND user_id=$2",
    )
    .bind(session.user_id)
    .bind(*target)
    .bind(&body.nickname)
    .bind(body.favorite)
    .bind(body.blocked)
    .bind(body.details.as_ref().map(Json))
    .execute(&mut *tx)
    .await?;
    if let Some(ids) = &category_ids {
        let count: i64 =
            sqlx::query_scalar("SELECT count(*) FROM categories WHERE owner_id=$1 AND id=ANY($2)")
                .bind(session.user_id)
                .bind(ids)
                .fetch_one(&mut *tx)
                .await?;
        if count != ids.len() as i64 {
            return Err(Error::forbidden());
        }
        sqlx::query("DELETE FROM contact_categories WHERE owner_id=$1 AND user_id=$2")
            .bind(session.user_id)
            .bind(*target)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO contact_categories(owner_id,user_id,category_id) SELECT $1,$2,unnest($3::uuid[])")
            .bind(session.user_id).bind(*target).bind(ids).execute(&mut *tx).await?;
    }
    let affected: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT user_id FROM conversation_members
        WHERE conversation_id IN(SELECT a.conversation_id FROM conversation_members a
            JOIN conversation_members b USING(conversation_id) WHERE a.user_id=$1 AND b.user_id=$2)
        UNION SELECT $1 UNION SELECT $2",
    )
    .bind(session.user_id)
    .bind(*target)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    changed(&state, affected).await;
    if body.blocked == Some(true) {
        state
            .publish(
                vec![session.user_id],
                ServerEvent::Presence {
                    user_id: *target,
                    presence: Presence::Offline,
                },
            )
            .await;
        state
            .publish(
                vec![*target],
                ServerEvent::Presence {
                    user_id: session.user_id,
                    presence: Presence::Offline,
                },
            )
            .await;
    } else if body.blocked == Some(false) || body.accepted == Some(true) {
        let _ = presence::announce(&state, session.user_id).await;
        let _ = presence::announce(&state, *target).await;
    }
    Ok(web::Json(one(&state, session.user_id, *target).await?))
}

async fn delete(
    state: web::Data<AppState>,
    session: Session,
    target: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    // Only the organizer's contact metadata is deleted. Independent blocking
    // edges and the other organizer's notes/categories remain intact.
    let count = sqlx::query("DELETE FROM contacts WHERE owner_id=$1 AND user_id=$2")
        .bind(session.user_id)
        .bind(*target)
        .execute(&mut *tx)
        .await?
        .rows_affected();
    if count == 0 {
        return Err(Error::missing());
    }
    sqlx::query("UPDATE contacts SET state='pending_outgoing' WHERE owner_id=$2 AND user_id=$1")
        .bind(session.user_id)
        .bind(*target)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    changed(&state, vec![session.user_id, *target]).await;
    state
        .publish(
            vec![session.user_id],
            ServerEvent::Presence {
                user_id: *target,
                presence: Presence::Offline,
            },
        )
        .await;
    state
        .publish(
            vec![*target],
            ServerEvent::Presence {
                user_id: session.user_id,
                presence: Presence::Offline,
            },
        )
        .await;
    Ok(HttpResponse::NoContent().finish())
}

async fn import(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<ImportContactsRequest>,
) -> Result<web::Json<Vec<Contact>>> {
    if body.emails.len() > 500 {
        return Err(Error::bad("Máximo de 500 contatos por importação"));
    }
    state
        .rate(&format!("import:{}", session.user_id), 5, 3600)
        .await?;
    let emails: HashSet<String> = body
        .emails
        .iter()
        .map(|e| validation::email(e))
        .collect::<Result<_>>()?;
    let mut tx = state.db.begin().await?;
    authz::exclusive(&mut tx).await?;
    let targets: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM users WHERE email=ANY($1) AND id<>$2")
            .bind(emails.into_iter().collect::<Vec<_>>())
            .bind(session.user_id)
            .fetch_all(&mut *tx)
            .await?;
    for target in &targets {
        add_in_tx(&mut tx, session.user_id, *target, None).await?;
    }
    tx.commit().await?;
    let mut recipients = targets.clone();
    recipients.push(session.user_id);
    changed(&state, recipients).await;
    let mut found: HashMap<_, _> = list(&state, session.user_id)
        .await?
        .into_iter()
        .map(|c| (c.user.id, c))
        .collect();
    Ok(web::Json(
        targets
            .into_iter()
            .filter_map(|id| found.remove(&id))
            .collect(),
    ))
}

async fn create_category(
    state: web::Data<AppState>,
    session: Session,
    body: web::Json<CategoryRequest>,
) -> Result<web::Json<Category>> {
    validation::text(body.name.trim(), "Categoria", 1, 100)?;
    let mut tx = state.db.begin().await?;
    // Owner row serializes position assignment and the per-owner resource bound.
    sqlx::query("SELECT id FROM users WHERE id=$1 FOR UPDATE")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM categories WHERE owner_id=$1")
        .bind(session.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= 100 {
        return Err(Error::bad("Máximo de 100 categorias"));
    }
    let (id,name,position): (Uuid,String,i32) = sqlx::query_as("INSERT INTO categories(id,owner_id,name,position)
        SELECT $1,$2,$3,COALESCE(max(position)+1,0) FROM categories WHERE owner_id=$2 RETURNING id,name,position")
        .bind(Uuid::new_v4()).bind(session.user_id).bind(body.name.trim()).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    changed(&state, vec![session.user_id]).await;
    Ok(web::Json(Category { id, name, position }))
}

async fn update_category(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
    body: web::Json<CategoryRequest>,
) -> Result<web::Json<Category>> {
    validation::text(body.name.trim(), "Categoria", 1, 100)?;
    let (id, name, position): (Uuid, String, i32) = sqlx::query_as(
        "UPDATE categories SET name=$3
        WHERE owner_id=$1 AND id=$2 RETURNING id,name,position",
    )
    .bind(session.user_id)
    .bind(*id)
    .bind(body.name.trim())
    .fetch_optional(&state.db)
    .await?
    .ok_or_else(Error::missing)?;
    changed(&state, vec![session.user_id]).await;
    Ok(web::Json(Category { id, name, position }))
}

async fn delete_category(
    state: web::Data<AppState>,
    session: Session,
    id: web::Path<Uuid>,
) -> Result<HttpResponse> {
    let result = sqlx::query("DELETE FROM categories WHERE owner_id=$1 AND id=$2")
        .bind(session.user_id)
        .bind(*id)
        .execute(&state.db)
        .await?;
    if result.rows_affected() == 0 {
        return Err(Error::missing());
    }
    changed(&state, vec![session.user_id]).await;
    Ok(HttpResponse::NoContent().finish())
}

pub fn routes(cfg: &mut web::ServiceConfig) {
    cfg.route("/contacts/import", web::post().to(import))
        .route("/contacts/export", web::get().to(get))
        .service(
            web::resource("/contacts")
                .route(web::get().to(get))
                .route(web::post().to(add)),
        )
        .service(
            web::resource("/contacts/{user_id}")
                .route(web::patch().to(update))
                .route(web::delete().to(delete)),
        )
        .route("/categories", web::post().to(create_category))
        .service(
            web::resource("/categories/{id}")
                .route(web::put().to(update_category))
                .route(web::delete().to(delete_category)),
        );
}
