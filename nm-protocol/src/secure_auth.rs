//! Shared OPAQUE cipher suite for password authentication without sending passwords.
pub use opaque_ke;
pub struct CipherSuite;

impl opaque_ke::CipherSuite for CipherSuite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeGroup = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::key_exchange::tripledh::TripleDh;
    type Ksf = argon2::Argon2<'static>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use opaque_ke::{ClientLogin, ClientLoginFinishParameters, ClientRegistration,
        ClientRegistrationFinishParameters, ServerLogin, ServerLoginStartParameters,
        ServerRegistration, ServerSetup};

    #[test]
    fn opaque_recovery_key_is_stable_and_wrong_password_fails() {
        let mut rng = opaque_ke::rand::rngs::OsRng;
        let setup = ServerSetup::<CipherSuite>::new(&mut rng);
        let start = ClientRegistration::<CipherSuite>::start(&mut rng, b"correct password").unwrap();
        let response = ServerRegistration::<CipherSuite>::start(&setup, start.message, b"account-id").unwrap();
        let finish = start.state.finish(&mut rng, b"correct password", response.message, ClientRegistrationFinishParameters::default()).unwrap();
        let file = ServerRegistration::<CipherSuite>::finish(finish.message);
        let login = ClientLogin::<CipherSuite>::start(&mut rng, b"correct password").unwrap();
        let response = ServerLogin::start(&mut rng, &setup, Some(file.clone()), login.message, b"account-id", ServerLoginStartParameters::default()).unwrap();
        let result = login.state.finish(b"correct password", response.message, ClientLoginFinishParameters::default()).unwrap();
        response.state.finish(result.message).unwrap();
        assert_eq!(finish.export_key, result.export_key);
        let login = ClientLogin::<CipherSuite>::start(&mut rng, b"wrong password").unwrap();
        let response = ServerLogin::start(&mut rng, &setup, Some(file), login.message, b"account-id", ServerLoginStartParameters::default()).unwrap();
        assert!(login.state.finish(b"wrong password", response.message, ClientLoginFinishParameters::default()).is_err());
    }
}
