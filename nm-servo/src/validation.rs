use crate::error::{Error, Result};
use nm_protocol::{MessageStyle, UserSettings};
use serde::{Serialize, de::DeserializeOwned};

pub fn text(value: &str, name: &str, min: usize, max: usize) -> Result<()> {
    let len = value.chars().count();
    if len < min || len > max || value.contains('\0') {
        return Err(Error::bad(format!("{name}: tamanho permitido {min}–{max}")));
    }
    Ok(())
}

pub fn email(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_lowercase();
    let parts: Vec<_> = value.split('@').collect();
    if value.len() > 254
        || !value.is_ascii()
        || parts.len() != 2
        || parts[0].is_empty()
        || parts[0].len() > 64
        || !parts[1].contains('.')
        || parts[1].starts_with('.')
        || parts[1].ends_with('.')
        || value
            .bytes()
            .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
    {
        return Err(Error::bad("E-mail inválido"));
    }
    Ok(value)
}

pub fn password(value: &str) -> Result<()> {
    if value.len() < 8 || value.len() > 1024 {
        return Err(Error::bad("Senha deve ter 8–1024 bytes"));
    }
    Ok(())
}

pub fn style(style: &MessageStyle) -> Result<()> {
    text(&style.font_family, "Fonte", 1, 100)?;
    if !style.font_size.is_finite() || !(6.0..=72.0).contains(&style.font_size) {
        return Err(Error::bad("Tamanho de fonte inválido"));
    }
    Ok(())
}

pub fn settings(value: &UserSettings) -> Result<()> {
    style(&value.font)?;
    text(&value.theme, "Tema", 1, 80)?;
    text(&value.receive_directory, "Diretório", 0, 1024)?;
    if value.auto_away_minutes > 1440 {
        return Err(Error::bad("Ausência automática inválida"));
    }
    Ok(())
}

pub fn json_size(value: &impl Serialize, max: usize) -> Result<()> {
    if serde_json::to_vec(value)
        .map_err(|_| Error::bad("JSON inválido"))?
        .len()
        > max
    {
        return Err(Error::too_large());
    }
    Ok(())
}

pub fn wire<T: Serialize>(value: T) -> Result<String> {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or_else(Error::internal)
}
pub fn parse<T: DeserializeOwned>(value: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(value.into())).map_err(|_| Error::internal())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_bad_inputs_and_normalizes_identity() {
        assert_eq!(email(" A@Example.COM ").unwrap(), "a@example.com");
        for bad in ["a@x", "@x.com", "a@@x.com", "a\n@x.com", "a@.com"] {
            assert!(email(bad).is_err());
        }
        assert!(password("short").is_err());
        let font = MessageStyle {
            font_size: f32::INFINITY,
            ..Default::default()
        };
        assert!(style(&font).is_err());
    }
}
