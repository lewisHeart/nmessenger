use super::*;

#[test]
fn mobile_active_conversation_can_be_marked_read_even_if_previously_detached() {
    let mut app = MessengerApp::new_for_test();
    let id = Uuid::new_v4();
    app.active = Some(id);
    app.detached.insert(id);
    assert_eq!(app.visible_conversation_for_read(true, true), Some(id));
    assert_eq!(app.visible_conversation_for_read(true, false), None);
    assert_eq!(app.visible_conversation_for_read(false, true), None);
}

#[test]
fn humanize_error_replaces_technical_and_http_codes_with_friendly_portuguese() {
    let login_err = MessengerApp::humanize_error(
        "unauthorized · Sessão inválida ou expirada (HTTP 401 Unauthorized)",
        false,
    );
    assert_eq!(
        login_err,
        crate::i18n::tr("Incorrect email or password. Check your credentials.", "E-mail ou senha incorretos. Verifique suas credenciais.")
    );
    let session_err = MessengerApp::humanize_error(
        "unauthorized · Sessão inválida ou expirada (HTTP 401 Unauthorized)",
        true,
    );
    assert_eq!(session_err, crate::i18n::tr("Your session has expired. Please sign in again.", "Sua sessão expirou. Conecte-se novamente."));
    let forbidden_err =
        MessengerApp::humanize_error("forbidden · Permissão negada (HTTP 403 Forbidden)", true);
    assert_eq!(forbidden_err, crate::i18n::tr("You do not have permission for this operation.", "Acesso não permitido para esta operação."));
    let clean_err = MessengerApp::humanize_error("invalid · O nome é muito curto", true);
    assert_eq!(clean_err, "O nome é muito curto");
}

#[test]
fn conversation_window_title_formats_direct_with_email_and_group() {
    let mut app = MessengerApp::new_for_test();
    let my_id = Uuid::new_v4();
    let other_id = Uuid::new_v4();
    app.data = Some(Bootstrap {
        me: Profile {
            id: my_id,
            email: "me@nmessenger.com".into(),
            display_name: "Eu".into(),
            personal_message: String::new(),
            presence: Presence::Available,
            avatar_id: None,
            scene: String::new(),
            now_playing: None,
            created_at: chrono::Utc::now(),
            bio: String::new(),
            badges: vec![],
        },
        contacts: vec![],
        categories: vec![],
        conversations: vec![],
        settings: UserSettings::default(),
        sessions: vec![],
        assets: vec![],
        activities: vec![],
        conversation_invites: vec![],
    });
    let direct_conv = Conversation {
        id: Uuid::new_v4(),
        owner_id: None,
        title: None,
        kind: ConversationKind::Direct,
        members: vec![
            app.data.as_ref().unwrap().me.clone(),
            Profile {
                id: other_id,
                email: "welton@example.com".into(),
                display_name: "Welton".into(),
                personal_message: String::new(),
                presence: Presence::Available,
                avatar_id: None,
                scene: String::new(),
                now_playing: None,
                created_at: chrono::Utc::now(),
                bio: String::new(),
                badges: vec![],
            },
        ],
        members_can_invite: true,
        updated_at: chrono::Utc::now(),
        unread: 0,
        last_message: None,
    };
    assert_eq!(
        app.conversation_window_title(&direct_conv),
        "Welton (welton@example.com) - Conversa"
    );
    let group_conv = Conversation {
        id: Uuid::new_v4(),
        owner_id: None,
        title: Some("Galera do MSN".into()),
        kind: ConversationKind::Group,
        members: vec![],
        members_can_invite: true,
        updated_at: chrono::Utc::now(),
        unread: 0,
        last_message: None,
    };
    assert_eq!(
        app.conversation_window_title(&group_conv),
        "Galera do MSN - Conversa"
    );
}
