//! Estado de la sección privada. Mientras está desbloqueada, la conexión SQLCipher abierta
//! es lo único que guarda la clave; al bloquear se cierra (SQLCipher pone a cero su memoria).
pub mod crypto;

use crate::db::private;
use crate::error::{AppError, Result};
use crypto::{Kdf, VaultFile};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};
use zeroize::Zeroizing;

/// Sección desbloqueada: la conexión SQLCipher y la DEK (esta última solo para cifrar copias de
/// seguridad; se pone a cero al bloquear).
struct Open {
    conn: Connection,
    dek: crypto::Key,
}

/// Espera progresiva tras fallos de contraseña o clave de recuperación: los 4 primeros intentos no
/// esperan; desde el 5.º se espera 5, 10, 20, 40 y 60 s (máximo). Un acierto reinicia la cuenta.
/// Solo frena el uso de la propia aplicación; no protege contra quien copia los ficheros (eso lo
/// hace el coste de Argon2id). Está en memoria: reiniciar la aplicación la reinicia.
#[derive(Default)]
struct Throttle {
    fails: u32,
    until: Option<Instant>,
}

impl Throttle {
    const FREE_TRIES: u32 = 4;
    const MAX_WAIT_SECS: u64 = 60;

    fn check(&self, now: Instant) -> Result<()> {
        match self.until {
            Some(t) if t > now => {
                let secs = (t - now).as_secs() + 1;
                Err(AppError::Auth(format!(
                    "Demasiados intentos fallidos. Espera {secs} s antes de volver a probar"
                )))
            }
            _ => Ok(()),
        }
    }

    fn failed(&mut self, now: Instant) {
        self.fails += 1;
        if self.fails > Self::FREE_TRIES {
            let exp = (self.fails - Self::FREE_TRIES - 1).min(10);
            let wait = (5u64 << exp).min(Self::MAX_WAIT_SECS);
            self.until = Some(now + Duration::from_secs(wait));
        }
    }

    fn succeeded(&mut self) {
        *self = Throttle::default();
    }
}

pub struct Vault {
    dir: PathBuf,
    kdf: Kdf,
    conn: Mutex<Option<Open>>,
    throttle: Mutex<Throttle>,
}

impl Vault {
    pub fn new(dir: PathBuf, kdf: Kdf) -> Self {
        Vault {
            dir,
            kdf,
            conn: Mutex::new(None),
            throttle: Mutex::new(Throttle::default()),
        }
    }

    /// Ejecuta una comprobación de credencial respetando y actualizando la espera progresiva.
    /// Solo cuentan como fallo los errores de autenticación.
    fn guarded<T>(&self, f: impl FnOnce() -> Result<T>) -> Result<T> {
        let mut t = self.throttle.lock().unwrap_or_else(|e| e.into_inner());
        t.check(Instant::now())?;
        let r = f();
        match &r {
            Ok(_) => t.succeeded(),
            Err(AppError::Auth(_)) => t.failed(Instant::now()),
            Err(_) => {}
        }
        r
    }

    fn vault_path(&self) -> PathBuf {
        self.dir.join("vault.json")
    }

    fn guard(&self) -> MutexGuard<'_, Option<Open>> {
        self.conn.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn open(&self, dek: &crypto::Key) -> Result<()> {
        let conn = private::open(&self.dir.join("private.db"), dek)?;
        *self.guard() = Some(Open {
            conn,
            dek: dek.clone(),
        });
        Ok(())
    }

    /// "none" (sin crear), "locked" o "unlocked".
    pub fn status(&self) -> &'static str {
        if self.guard().is_some() {
            "unlocked"
        } else if self.vault_path().exists() {
            "locked"
        } else {
            "none"
        }
    }

    /// Crea la contraseña maestra y deja la sección desbloqueada. Devuelve la clave de recuperación.
    pub fn create(&self, password: &str) -> Result<Zeroizing<String>> {
        if self.vault_path().exists() {
            return Err(AppError::Validation(
                "La sección privada ya está creada".into(),
            ));
        }
        let (file, dek, recovery) = crypto::create(password, self.kdf)?;
        file.save(&self.vault_path())?;
        self.open(&dek)?;
        Ok(recovery)
    }

    pub fn unlock(&self, password: &str) -> Result<()> {
        let dek =
            self.guarded(|| VaultFile::load(&self.vault_path())?.unlock_password(password))?;
        self.open(&dek)
    }

    /// Contraseña olvidada: la clave de recuperación abre la DEK y se fija una contraseña nueva.
    pub fn reset_password(&self, recovery: &str, new_password: &str) -> Result<()> {
        let file = VaultFile::load(&self.vault_path())?;
        let dek = self.guarded(|| file.unlock_recovery(recovery))?;
        file.with_new_password(&dek, new_password)?
            .save(&self.vault_path())?;
        self.open(&dek)
    }

    pub fn change_password(&self, old: &str, new: &str) -> Result<()> {
        let file = VaultFile::load(&self.vault_path())?;
        let dek = self.guarded(|| file.unlock_password(old))?;
        file.with_new_password(&dek, new)?.save(&self.vault_path())
    }

    pub fn lock(&self) {
        if let Some(o) = self.guard().take() {
            let _ = o.conn.close(); // `o.dek` se pone a cero al soltarse
        }
    }

    /// Ejecuta `f` con la conexión privada; error `Locked` si no está desbloqueada.
    pub fn with<T>(&self, f: impl FnOnce(&Connection) -> Result<T>) -> Result<T> {
        match self.guard().as_ref() {
            Some(o) => f(&o.conn),
            None => Err(AppError::Locked("La sección privada está bloqueada".into())),
        }
    }

    /// Ejecuta `f` con la DEK (solo desbloqueada). Para cifrar copias de seguridad.
    pub fn with_dek<T>(&self, f: impl FnOnce(&crypto::Key) -> Result<T>) -> Result<T> {
        match self.guard().as_ref() {
            Some(o) => f(&o.dek),
            None => Err(AppError::Locked(
                "Desbloquea la sección privada para crear copias de seguridad".into(),
            )),
        }
    }

    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// Contenido de vault.json (envuelve la DEK; no contiene secretos en claro).
    pub fn vault_bytes(&self) -> Result<Vec<u8>> {
        std::fs::read(self.vault_path())
            .map_err(|_| AppError::Vault("No se pudo leer la configuración de seguridad".into()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::private::PrivateData;
    use std::collections::HashSet;

    const PW: &str = "correcta-caballo-1";

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("od-vault-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn person() -> PrivateData {
        PrivateData {
            name: Some("Persona de Prueba".into()),
            email: Some("prueba@example.invalid".into()),
            phone: Some("+00 000 000".into()),
            address: Some("Calle Falsa 123".into()),
            country: Some("Chile".into()),
            region: Some("Región de Prueba".into()),
            city: Some("Ciudad de Prueba".into()),
            postal_code: Some("0000000".into()),
            delivery_notes: Some("Dejar con conserjería".into()),
            rut: Some("11111111-1".into()),
            business_name: Some("Empresa de Prueba SpA".into()),
            business_activity: Some("Venta de pruebas".into()),
        }
    }

    #[test]
    fn full_lifecycle() {
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::FAST);
        assert_eq!(v.status(), "none");
        assert!(matches!(v.with(|_| Ok(())), Err(AppError::Locked(_))));

        let rec = v.create(PW).unwrap();
        assert_eq!(v.status(), "unlocked");
        v.with(|c| private::set(c, "o1", person())).unwrap();
        assert_eq!(v.with(|c| private::get(c, "o1")).unwrap(), Some(person()));

        v.lock();
        assert_eq!(v.status(), "locked");
        assert!(matches!(
            v.with(|c| private::get(c, "o1")),
            Err(AppError::Locked(_))
        ));

        assert!(matches!(v.unlock("incorrecta-123"), Err(AppError::Auth(_))));
        assert_eq!(v.status(), "locked");
        v.unlock(PW).unwrap();
        assert_eq!(v.with(|c| private::get(c, "o1")).unwrap(), Some(person()));

        // cambio de contraseña: la vieja deja de valer, los datos siguen
        assert!(matches!(
            v.change_password("incorrecta-123", "nueva-contraseña-2"),
            Err(AppError::Auth(_))
        ));
        v.change_password(PW, "nueva-contraseña-2").unwrap();
        v.lock();
        assert!(v.unlock(PW).is_err());
        v.unlock("nueva-contraseña-2").unwrap();
        assert_eq!(v.with(|c| private::get(c, "o1")).unwrap(), Some(person()));

        // contraseña olvidada: la clave de recuperación abre y fija una nueva
        v.lock();
        v.reset_password(&rec, "tercera-contraseña-3").unwrap();
        assert_eq!(v.status(), "unlocked");
        v.lock();
        assert!(v.unlock("nueva-contraseña-2").is_err());
        v.unlock("tercera-contraseña-3").unwrap();
        assert_eq!(v.with(|c| private::get(c, "o1")).unwrap(), Some(person()));
        v.lock(); // Windows no borra ficheros abiertos
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn private_db_is_encrypted_and_vault_is_not_recreated() {
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::FAST);
        v.create(PW).unwrap();
        v.with(|c| private::set(c, "o1", person())).unwrap();
        v.lock();
        let raw = std::fs::read(dir.join("private.db")).unwrap();
        assert!(!raw.starts_with(b"SQLite format 3"));
        for needle in [
            "Persona de Prueba",
            "prueba@example.invalid",
            "Calle Falsa",
            "order_private",
        ] {
            assert!(
                !raw.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "texto en claro: {needle}"
            );
        }
        assert!(matches!(v.create(PW), Err(AppError::Validation(_))));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn private_db_is_unreadable_with_another_key() {
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::FAST);
        v.create(PW).unwrap();
        v.lock();
        let other: crypto::Key = Zeroizing::new([7u8; 32]);
        assert!(matches!(
            private::open(&dir.join("private.db"), &other),
            Err(AppError::Auth(_))
        ));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn private_data_rules() {
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::FAST);
        v.create(PW).unwrap();
        // vacío borra; espacios se recortan; límites validados
        v.with(|c| private::set(c, "o1", person())).unwrap();
        v.with(|c| {
            private::set(
                c,
                "o1",
                PrivateData {
                    name: Some("  ".into()),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        assert_eq!(v.with(|c| private::get(c, "o1")).unwrap(), None);
        v.with(|c| {
            private::set(
                c,
                "o2",
                PrivateData {
                    name: Some("  Ana  ".into()),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        assert_eq!(
            v.with(|c| private::get(c, "o2"))
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Ana")
        );
        let long = PrivateData {
            phone: Some("1".repeat(51)),
            ..Default::default()
        };
        assert!(matches!(
            v.with(|c| private::set(c, "o2", long)),
            Err(AppError::Validation(_))
        ));

        // huérfanos: solo se conservan los pedidos que siguen existiendo
        v.with(|c| private::set(c, "o3", person())).unwrap();
        let keep: HashSet<String> = ["o2".to_string()].into();
        assert_eq!(v.with(|c| private::purge_orphans(c, &keep)).unwrap(), 1);
        assert!(v.with(|c| private::get(c, "o3")).unwrap().is_none());
        assert!(v.with(|c| private::get(c, "o2")).unwrap().is_some());
        v.lock();
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn business_db_has_no_personal_columns() {
        // Garantía de diseño: ninguna tabla de business.db tiene columnas de datos personales.
        let c = crate::db::open_in_memory();
        let tables: Vec<String> = c
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<std::result::Result<_, _>>()
            .unwrap();
        for t in tables {
            let cols: Vec<String> = c
                .prepare(&format!("PRAGMA table_info({t})"))
                .unwrap()
                .query_map([], |r| r.get::<_, String>(1))
                .unwrap()
                .collect::<std::result::Result<_, _>>()
                .unwrap();
            for col in cols {
                for banned in ["email", "phone", "address", "customer", "buyer"] {
                    assert!(!col.to_lowercase().contains(banned), "{t}.{col}");
                }
            }
        }
    }

    #[test]
    fn failed_attempts_back_off_and_success_resets() {
        let t0 = Instant::now();
        let mut t = Throttle::default();
        for _ in 0..4 {
            assert!(t.check(t0).is_ok());
            t.failed(t0);
        }
        assert!(t.check(t0).is_ok()); // los 4 primeros fallos no esperan
        t.failed(t0); // 5.º fallo: 5 s
        assert!(matches!(
            t.check(t0 + Duration::from_secs(4)),
            Err(AppError::Auth(_))
        ));
        assert!(t.check(t0 + Duration::from_secs(5)).is_ok());
        let waits: Vec<u64> = (0..6)
            .map(|i| {
                t.failed(t0);
                let mut s = 0;
                while t.check(t0 + Duration::from_secs(s)).is_err() {
                    s += 1;
                }
                let _ = i;
                s
            })
            .collect();
        assert_eq!(waits, [10, 20, 40, 60, 60, 60]); // crece y se queda en 60 s
        t.succeeded();
        assert!(t.check(t0).is_ok());
        t.failed(t0);
        assert!(t.check(t0).is_ok()); // la cuenta empezó de cero
    }

    #[test]
    fn vault_blocks_after_repeated_wrong_passwords_but_not_other_errors() {
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::FAST);
        v.create(PW).unwrap();
        v.lock();
        for _ in 0..4 {
            assert!(matches!(v.unlock("incorrecta-123"), Err(AppError::Auth(_))));
        }
        assert!(v.unlock("incorrecta-123").is_err()); // 5.º fallo
        // Ahora ni siquiera la contraseña correcta entra hasta que pase la espera.
        let e = v.unlock(PW).unwrap_err().to_string();
        assert!(e.contains("Espera"), "{e}");
        assert_eq!(v.status(), "locked");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
