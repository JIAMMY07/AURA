use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use sha2::Sha256;

const SERVICE: &str = "local.aura.assistant";
const ACCOUNT: &str = "database-master-key-v1";

pub struct Crypto {
    encryption_key: [u8; 32],
    audit_key: [u8; 32],
}

impl Crypto {
    pub fn load_or_create() -> Result<Self, String> {
        let entry = keyring::Entry::new(SERVICE, ACCOUNT).map_err(redact_error)?;
        let master = match entry.get_password() {
            Ok(encoded) => decode_master_key(&encoded)?,
            Err(keyring::Error::NoEntry) => {
                let generated = Aes256Gcm::generate_key(&mut OsRng);
                let mut master = [0_u8; 32];
                master.copy_from_slice(generated.as_slice());
                entry
                    .set_password(&STANDARD.encode(master))
                    .map_err(redact_error)?;
                master
            }
            Err(error) => return Err(redact_error(error)),
        };
        Self::from_master(master)
    }

    pub(crate) fn from_master(master: [u8; 32]) -> Result<Self, String> {
        let hkdf = Hkdf::<Sha256>::new(None, &master);
        let mut encryption_key = [0_u8; 32];
        let mut audit_key = [0_u8; 32];
        hkdf.expand(b"aura-content-encryption-v1", &mut encryption_key)
            .map_err(|_| "Impossibile derivare la chiave di cifratura.".to_string())?;
        hkdf.expand(b"aura-audit-authentication-v1", &mut audit_key)
            .map_err(|_| "Impossibile derivare la chiave di audit.".to_string())?;
        Ok(Self { encryption_key, audit_key })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<(Vec<u8>, Vec<u8>), String> {
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|_| "Chiave di cifratura non valida.".to_string())?;
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| "Cifratura del messaggio fallita.".to_string())?;
        Ok((ciphertext, nonce.to_vec()))
    }

    pub fn decrypt(&self, ciphertext: &[u8], nonce: &[u8]) -> Result<String, String> {
        if nonce.len() != 12 {
            return Err("Nonce cifrato non valido.".to_string());
        }
        let cipher = Aes256Gcm::new_from_slice(&self.encryption_key)
            .map_err(|_| "Chiave di cifratura non valida.".to_string())?;
        let plaintext = cipher
            .decrypt(Nonce::from_slice(nonce), ciphertext)
            .map_err(|_| "Messaggio cifrato non autenticabile.".to_string())?;
        String::from_utf8(plaintext).map_err(|_| "Messaggio cifrato non UTF-8.".to_string())
    }

    pub fn hmac_hex(&self, data: &[u8]) -> Result<String, String> {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(&self.audit_key)
            .map_err(|_| "Chiave HMAC non valida.".to_string())?;
        mac.update(data);
        Ok(hex::encode(mac.finalize().into_bytes()))
    }

    pub fn verify_hmac_hex(&self, data: &[u8], encoded: &str) -> bool {
        let Ok(expected) = hex::decode(encoded) else { return false };
        let Ok(mut mac) = <Hmac<Sha256> as Mac>::new_from_slice(&self.audit_key) else { return false };
        mac.update(data);
        mac.verify_slice(&expected).is_ok()
    }
}

fn decode_master_key(encoded: &str) -> Result<[u8; 32], String> {
    let decoded = STANDARD
        .decode(encoded)
        .map_err(|_| "Chiave Windows memorizzata non valida.".to_string())?;
    decoded
        .try_into()
        .map_err(|_| "Lunghezza chiave Windows non valida.".to_string())
}

fn redact_error(error: keyring::Error) -> String {
    format!("Archivio credenziali Windows non disponibile: {}", error)
}

#[cfg(test)]
mod tests {
    use super::Crypto;

    #[test]
    fn encrypts_and_authenticates_round_trip() {
        let crypto = Crypto::from_master([7_u8; 32]).expect("valid test key");
        let (ciphertext, nonce) = crypto.encrypt("segreto locale").expect("encrypt");
        assert_ne!(ciphertext, b"segreto locale");
        assert_eq!(crypto.decrypt(&ciphertext, &nonce).expect("decrypt"), "segreto locale");
    }

    #[test]
    fn rejects_tampered_ciphertext() {
        let crypto = Crypto::from_master([8_u8; 32]).expect("valid test key");
        let (mut ciphertext, nonce) = crypto.encrypt("segreto").expect("encrypt");
        ciphertext[0] ^= 1;
        assert!(crypto.decrypt(&ciphertext, &nonce).is_err());
    }

    #[test]
    fn verifies_audit_mac() {
        let crypto = Crypto::from_master([9_u8; 32]).expect("valid test key");
        let mac = crypto.hmac_hex(b"evento").expect("mac");
        assert!(crypto.verify_hmac_hex(b"evento", &mac));
        assert!(!crypto.verify_hmac_hex(b"evento alterato", &mac));
    }
}
