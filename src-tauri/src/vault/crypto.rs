//! Formato de `vault.json` y manejo de claves. Solo usa crates auditadas (RustCrypto):
//! Argon2id para derivar la clave de la contraseña y XChaCha20-Poly1305 para envolver la DEK.
//! La DEK (256 bits aleatorios) abre private.db; cambiar la contraseña solo re-envuelve la DEK.
use crate::error::{AppError, Result};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use serde::{Deserialize, Serialize};
use std::path::Path;
use zeroize::{Zeroize, Zeroizing};

pub type Key = Zeroizing<[u8; 32]>;

const VERSION: u32 = 1;
const AAD_PASSWORD: &[u8] = b"order-desk/vault/v1/password";
const AAD_RECOVERY: &[u8] = b"order-desk/vault/v1/recovery";
pub const MIN_PASSWORD_CHARS: usize = 10;

#[derive(Clone, Copy, Serialize, Deserialize)]
pub struct Kdf {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

/// Parámetros de producción. Se guardan en vault.json, así que se pueden subir en el futuro.
pub const KDF: Kdf = Kdf {
    m_kib: 64 * 1024,
    t: 3,
    p: 1,
};

#[derive(Serialize, Deserialize)]
struct Slot {
    nonce: String,
    ct: String,
}

#[derive(Serialize, Deserialize)]
pub struct VaultFile {
    version: u32,
    alg: String,
    kdf: Kdf,
    salt: String,
    password_slot: Slot,
    recovery_slot: Slot,
}

fn fail() -> AppError {
    AppError::Vault("No se pudo completar la operación de cifrado".into())
}

pub fn auth_error() -> AppError {
    AppError::Auth("Contraseña o clave incorrecta".into())
}

pub fn random<const N: usize>() -> Result<[u8; N]> {
    let mut b = [0u8; N];
    getrandom::fill(&mut b).map_err(|_| fail())?;
    Ok(b)
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 || !s.is_ascii() {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn derive(password: &str, salt: &[u8], kdf: &Kdf) -> Result<Key> {
    let params = Params::new(kdf.m_kib, kdf.t, kdf.p, Some(32)).map_err(|_| fail())?;
    let mut out = Zeroizing::new([0u8; 32]);
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, &mut *out)
        .map_err(|_| fail())?;
    Ok(out)
}

fn wrap(key: &Key, dek: &Key, aad: &[u8]) -> Result<Slot> {
    let nonce = random::<24>()?;
    let ct = XChaCha20Poly1305::new((&**key).into())
        .encrypt(XNonce::from_slice(&nonce), Payload { msg: &**dek, aad })
        .map_err(|_| fail())?;
    Ok(Slot {
        nonce: hex(&nonce),
        ct: hex(&ct),
    })
}

fn unwrap_dek(key: &Key, slot: &Slot, aad: &[u8]) -> Result<Key> {
    let (Some(nonce), Some(ct)) = (unhex(&slot.nonce), unhex(&slot.ct)) else {
        return Err(fail());
    };
    if nonce.len() != 24 {
        return Err(fail());
    }
    let mut plain = XChaCha20Poly1305::new((&**key).into())
        .decrypt(XNonce::from_slice(&nonce), Payload { msg: &ct, aad })
        .map_err(|_| auth_error())?;
    let mut dek = Zeroizing::new([0u8; 32]);
    if plain.len() != 32 {
        plain.zeroize();
        return Err(fail());
    }
    dek.copy_from_slice(&plain);
    plain.zeroize();
    Ok(dek)
}

/// Clave de recuperación legible: 32 bytes en hexadecimal, en grupos de 8.
fn format_recovery(raw: &[u8; 32]) -> Zeroizing<String> {
    let h = Zeroizing::new(hex(raw).to_uppercase());
    let groups: Vec<&str> = h
        .as_bytes()
        .chunks(8)
        .map(|c| std::str::from_utf8(c).unwrap_or(""))
        .collect();
    Zeroizing::new(groups.join("-"))
}

fn parse_recovery(text: &str) -> Result<Key> {
    let clean = Zeroizing::new(
        text.chars()
            .filter(|c| !c.is_whitespace() && *c != '-')
            .collect::<String>(),
    );
    let raw = Zeroizing::new(unhex(&clean).unwrap_or_default());
    if raw.len() != 32 {
        return Err(auth_error());
    }
    let mut k = Zeroizing::new([0u8; 32]);
    k.copy_from_slice(&raw);
    Ok(k)
}

pub fn check_new_password(pw: &str) -> Result<()> {
    if pw.chars().count() < MIN_PASSWORD_CHARS {
        return Err(AppError::Validation(format!(
            "La contraseña debe tener al menos {MIN_PASSWORD_CHARS} caracteres"
        )));
    }
    Ok(())
}

/// Crea un vault nuevo. Devuelve el fichero, la DEK y la clave de recuperación (se muestra una sola vez).
pub fn create(password: &str, kdf: Kdf) -> Result<(VaultFile, Key, Zeroizing<String>)> {
    check_new_password(password)?;
    let dek: Key = Zeroizing::new(random::<32>()?);
    let salt = random::<16>()?;
    let recovery_raw: Key = Zeroizing::new(random::<32>()?);
    let file = VaultFile {
        version: VERSION,
        alg: "argon2id+xchacha20poly1305".into(),
        kdf,
        salt: hex(&salt),
        password_slot: wrap(&derive(password, &salt, &kdf)?, &dek, AAD_PASSWORD)?,
        recovery_slot: wrap(&recovery_raw, &dek, AAD_RECOVERY)?,
    };
    Ok((file, dek, format_recovery(&recovery_raw)))
}

impl VaultFile {
    pub fn unlock_password(&self, password: &str) -> Result<Key> {
        let salt = unhex(&self.salt).ok_or_else(fail)?;
        unwrap_dek(
            &derive(password, &salt, &self.kdf)?,
            &self.password_slot,
            AAD_PASSWORD,
        )
    }

    pub fn unlock_recovery(&self, recovery: &str) -> Result<Key> {
        unwrap_dek(
            &parse_recovery(recovery)?,
            &self.recovery_slot,
            AAD_RECOVERY,
        )
    }

    /// Re-envuelve la DEK con una contraseña nueva (sal nueva); la clave de recuperación sigue valiendo.
    pub fn with_new_password(&self, dek: &Key, new_password: &str) -> Result<VaultFile> {
        check_new_password(new_password)?;
        let salt = random::<16>()?;
        Ok(VaultFile {
            version: self.version,
            alg: self.alg.clone(),
            kdf: self.kdf,
            salt: hex(&salt),
            password_slot: wrap(&derive(new_password, &salt, &self.kdf)?, dek, AAD_PASSWORD)?,
            recovery_slot: Slot {
                nonce: self.recovery_slot.nonce.clone(),
                ct: self.recovery_slot.ct.clone(),
            },
        })
    }

    /// Escritura atómica: fichero temporal + renombrado, para no dejar un vault a medias.
    pub fn save(&self, path: &Path) -> Result<()> {
        let json = serde_json::to_vec_pretty(self).map_err(|_| fail())?;
        let tmp = path.with_extension("json.tmp");
        let write = || -> std::io::Result<()> {
            use std::io::Write;
            let mut f = std::fs::File::create(&tmp)?;
            f.write_all(&json)?;
            f.sync_all()?;
            std::fs::rename(&tmp, path)
        };
        write()
            .map_err(|_| AppError::Vault("No se pudo guardar la configuración de seguridad".into()))
    }

    pub fn load(path: &Path) -> Result<VaultFile> {
        let bytes = std::fs::read(path)
            .map_err(|_| AppError::Vault("No se pudo leer la configuración de seguridad".into()))?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<VaultFile> {
        let f: VaultFile = serde_json::from_slice(bytes)
            .map_err(|_| AppError::Vault("La configuración de seguridad está dañada".into()))?;
        if f.version != VERSION {
            return Err(AppError::Vault(
                "Versión de seguridad no compatible con esta aplicación".into(),
            ));
        }
        Ok(f)
    }
}

/// Parámetros mínimos solo para que las pruebas no tarden.
#[cfg(test)]
pub const FAST: Kdf = Kdf {
    m_kib: 8,
    t: 1,
    p: 1,
};

#[cfg(test)]
mod tests {
    use super::*;

    const PW: &str = "correcta-caballo-1";

    #[test]
    fn password_and_recovery_open_the_same_dek() {
        let (f, dek, rec) = create(PW, FAST).unwrap();
        assert_eq!(*f.unlock_password(PW).unwrap(), *dek);
        assert_eq!(*f.unlock_recovery(&rec).unwrap(), *dek);
        // tolerante con espacios y minúsculas al teclearla
        let typed = rec.to_lowercase().replace('-', " ");
        assert_eq!(*f.unlock_recovery(&typed).unwrap(), *dek);
    }

    #[test]
    fn wrong_password_or_recovery_is_rejected() {
        let (f, _, rec) = create(PW, FAST).unwrap();
        assert!(matches!(
            f.unlock_password("otra-contraseña-1"),
            Err(AppError::Auth(_))
        ));
        assert!(matches!(f.unlock_recovery("00"), Err(AppError::Auth(_))));
        let mut bad = rec.to_string();
        bad.replace_range(0..1, if bad.starts_with('A') { "B" } else { "A" });
        assert!(matches!(f.unlock_recovery(&bad), Err(AppError::Auth(_))));
    }

    #[test]
    fn short_password_is_rejected() {
        assert!(matches!(
            create("corta", FAST),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn password_change_keeps_dek_and_recovery() {
        let (f, dek, rec) = create(PW, FAST).unwrap();
        let g = f.with_new_password(&dek, "nueva-contraseña-2").unwrap();
        assert!(g.unlock_password(PW).is_err());
        assert_eq!(*g.unlock_password("nueva-contraseña-2").unwrap(), *dek);
        assert_eq!(*g.unlock_recovery(&rec).unwrap(), *dek);
    }

    #[test]
    fn slots_are_bound_to_their_purpose() {
        // Un slot de contraseña no se puede abrir como slot de recuperación (AAD distinto).
        let (f, dek, _) = create(PW, FAST).unwrap();
        let salt = unhex(&f.salt).unwrap();
        let k = derive(PW, &salt, &f.kdf).unwrap();
        assert!(unwrap_dek(&k, &f.password_slot, AAD_RECOVERY).is_err());
        assert_eq!(
            *unwrap_dek(&k, &f.password_slot, AAD_PASSWORD).unwrap(),
            *dek
        );
    }

    #[test]
    fn file_roundtrip_has_no_secrets_in_plain_text() {
        let dir = std::env::temp_dir().join(format!("od-vault-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("vault.json");
        let (f, dek, rec) = create(PW, FAST).unwrap();
        f.save(&p).unwrap();
        let text = std::fs::read_to_string(&p).unwrap();
        assert!(!text.contains(PW) && !text.contains(&hex(&*dek)) && !text.contains(&*rec));
        assert!(!p.with_extension("json.tmp").exists());
        let g = VaultFile::load(&p).unwrap();
        assert_eq!(*g.unlock_password(PW).unwrap(), *dek);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
