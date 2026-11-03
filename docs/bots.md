# Bot SDK & Ecosystem

NMessenger includes native support for automated bot accounts. Bots integrate using the same end-to-end encrypted messaging protocol as human users.

---

## 1. Creating a Bot Account

Administrators and verified users can register bot accounts through the API:

```sh
# Create a new bot
curl -X POST https://YOUR_DOMAIN/v1/bots \
  -H "Authorization: Bearer YOUR_ADMIN_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name": "EchoBot", "display_name": "Echo Bot"}'
```

The response returns a permanent bot token:
```json
{
  "id": "c1f7a049-7c85-48b2-b43a-7efca2219760",
  "name": "EchoBot",
  "token": "bot_token_secret_here"
}
```

---

## 2. Using the Rust Bot SDK

The `nmessenger-client` crate includes a lightweight SDK for building automated bots and integrations:

```rust
use nmessenger_client::bot_sdk::{BotClient, BotHandler};
use nm_protocol::models::{Message, MessageKind};

struct MyBot;

#[async_trait::async_trait]
impl BotHandler for MyBot {
    async fn on_message(&self, bot: &BotClient, message: Message) {
        if let MessageKind::Text(content) = message.kind {
            if content.starts_with("!ping") {
                let _ = bot.send_text(message.conversation_id, "pong!").await;
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = BotClient::connect("https://messenger.example.com", "YOUR_BOT_TOKEN").await?;
    bot.run(MyBot).await?;
    Ok(())
}
```

---

## 3. Security Considerations for Bots

- Bots participate in end-to-end encryption using their own cryptographic device keys.
- Bot identity keys are exchanged through the normal public key directory.
- Bot tokens can be revoked or rotated at any time via `DELETE /v1/bots/{id}/token`.

