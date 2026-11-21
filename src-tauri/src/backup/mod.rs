//! Copias de seguridad cifradas (`.odbak`) y su restauración.
//!
//! Formato: `MAGIC | u32 len | vault.json | nonce(24) | XChaCha20-Poly1305(payload)`.
//! `vault.json` va en claro (solo contiene la DEK envuelta, ver `vault/crypto.rs`) para que la copia
//! se abra en otro equipo con la contraseña maestra del momento de la copia o con la clave de
//! recuperación. El payload (business.db + private.db) se cifra con una clave derivada de la DEK
//! con HKDF-SHA256 (otra clave distinta de la que usa SQLCipher). Cabecera y `vault.json` van como
//! datos asociados autenticados.
use crate::db;
use crate::db::private;
use crate::error::{AppError, Result};
use crate::vault::crypto::{Key, VaultFile, random};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use sha2::Sha256;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

const MAGIC: &[u8] = b"ORDERDESK-BACKUP\x01";
const NONCE_LEN: usize = 24;
const MAX_VAULT_JSON: usize = 64 * 1024;

pub enum Secret<'a> {
    Password(&'a str),
    Recovery(&'a str),
}

/// Contenido ya descifrado de una copia.
pub struct Opened {
    vault_json: Vec<u8>,
    business: Zeroizing<Vec<u8>>,
    private: Zeroizing<Vec<u8>>,
}

fn crypto_error() -> AppError {
    AppError::Io("No se pudo completar la operación de cifrado".into())
}

fn corrupt() -> AppError {
    AppError::Validation("El archivo no es una copia válida de Order Desk o está dañado".into())
}

fn backup_key(dek: &Key) -> Result<Key> {
    let mut out = Zeroizing::new([0u8; 32]);
    Hkdf::<Sha256>::new(None, &**dek)
        .expand(b"order-desk/backup/v1", &mut *out)
        .map_err(|_| crypto_error())?;
    Ok(out)
}

fn header(vault_json: &[u8]) -> Result<Vec<u8>> {
    let len = u32::try_from(vault_json.len()).map_err(|_| crypto_error())?;
    let mut h = MAGIC.to_vec();
    h.extend_from_slice(&len.to_le_bytes());
    h.extend_from_slice(vault_json);
    Ok(h)
}

fn pack(parts: &[&[u8]]) -> Zeroizing<Vec<u8>> {
    let mut out = Zeroizing::new(Vec::new());
    for p in parts {
        out.extend_from_slice(&(p.len() as u64).to_le_bytes());
        out.extend_from_slice(p);
    }
    out
}

fn unpack(mut bytes: &[u8]) -> Result<[Zeroizing<Vec<u8>>; 2]> {
    let mut take = || -> Result<Zeroizing<Vec<u8>>> {
        let (len, rest) = bytes.split_first_chunk::<8>().ok_or_else(corrupt)?;
        let len = usize::try_from(u64::from_le_bytes(*len)).map_err(|_| corrupt())?;
        if rest.len() < len {
            return Err(corrupt());
        }
        let (part, rest) = rest.split_at(len);
        bytes = rest;
        Ok(Zeroizing::new(part.to_vec()))
    };
    let a = take()?;
    let b = take()?;
    Ok([a, b])
}

/// Lee los dos ficheros de base de datos de la carpeta de datos. Quien llama debe tener tomados los
/// cierres de ambas bases para que no haya escrituras a medias (cada operación es una transacción).
pub fn snapshot(dir: &Path) -> Result<(Vec<u8>, Vec<u8>)> {
    let read = |n: &str| {
        std::fs::read(dir.join(n))
            .map_err(|_| AppError::Io("No se pudo leer la base de datos".into()))
    };
    Ok((read("business.db")?, read("private.db")?))
}

pub fn create(dek: &Key, vault_json: &[u8], business: &[u8], private: &[u8]) -> Result<Vec<u8>> {
    let header = header(vault_json)?;
    let nonce = random::<NONCE_LEN>()?;
    let plain = pack(&[business, private]);
    let ct = XChaCha20Poly1305::new((&*backup_key(dek)?).into())
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plain,
                aad: &header,
            },
        )
        .map_err(|_| crypto_error())?;
    let mut out = header;
    out.extend_from_slice(&nonce);
    out.extend_from_slice(&ct);
    Ok(out)
}

/// Descifra una copia con la contraseña o la clave de recuperación. Devuelve también la DEK.
pub fn open(bytes: &[u8], secret: Secret) -> Result<(Opened, Key)> {
    let rest = bytes.strip_prefix(MAGIC).ok_or_else(corrupt)?;
    let (len, rest) = rest.split_first_chunk::<4>().ok_or_else(corrupt)?;
    let len = u32::from_le_bytes(*len) as usize;
    if len > MAX_VAULT_JSON || rest.len() < len + NONCE_LEN {
        return Err(corrupt());
    }
    let (vault_json, rest) = rest.split_at(len);
    let (nonce, ct) = rest.split_at(NONCE_LEN);

    let file = VaultFile::from_bytes(vault_json).map_err(|_| corrupt())?;
    let dek = match secret {
        Secret::Password(p) => file.unlock_password(p)?,
        Secret::Recovery(r) => file.unlock_recovery(r)?,
    };
    let aad = header(vault_json)?;
    let plain = Zeroizing::new(
        XChaCha20Poly1305::new((&*backup_key(&dek)?).into())
            .decrypt(XNonce::from_slice(nonce), Payload { msg: ct, aad: &aad })
            .map_err(|_| corrupt())?,
    );
    let [business, private] = unpack(&plain)?;
    Ok((
        Opened {
            vault_json: vault_json.to_vec(),
            business,
            private,
        },
        dek,
    ))
}

const FILES: [&str; 3] = ["business.db", "private.db", "vault.json"];

fn side(dir: &Path, name: &str, suffix: &str) -> PathBuf {
    dir.join(format!("{name}.{suffix}"))
}

/// Pone el contenido de la copia en `dir`. Primero escribe y valida temporales; después renombra.
/// Los ficheros anteriores se conservan como `<nombre>.before-restore`. Las bases de `dir` no
/// pueden estar abiertas.
pub fn restore_into(dir: &Path, o: &Opened, dek: &Key) -> Result<()> {
    let io = |_| AppError::Io("No se pudo escribir la restauración".into());
    let data: [&[u8]; 3] = [&o.business, &o.private, &o.vault_json];

    let cleanup = || {
        for n in FILES {
            let _ = std::fs::remove_file(side(dir, n, "restore"));
        }
    };
    let staged = (|| -> Result<()> {
        for (n, d) in FILES.iter().zip(data) {
            std::fs::write(side(dir, n, "restore"), d).map_err(io)?;
        }
        // Validar: abrir también aplica las migraciones y falla si la copia es de una versión más nueva.
        db::open(&side(dir, "business.db", "restore"))?
            .close()
            .map_err(|(_, e)| AppError::Db(e))?;
        private::open(&side(dir, "private.db", "restore"), dek)?
            .close()
            .map_err(|(_, e)| AppError::Db(e))?;
        Ok(())
    })();
    if let Err(e) = staged {
        cleanup();
        return Err(e);
    }

    // Una restauración anterior no se pisa: se busca el primer sufijo libre para todos los ficheros.
    let tag = (1..)
        .map(|k| {
            if k == 1 {
                "before-restore".to_string()
            } else {
                format!("before-restore-{k}")
            }
        })
        .find(|t| FILES.iter().all(|n| !side(dir, n, t).exists()))
        .unwrap_or_default();
    let mut moved: Vec<&str> = vec![];
    let swap = (|| -> Result<()> {
        for n in FILES {
            let target = dir.join(n);
            let before = side(dir, n, &tag);
            if target.exists() {
                std::fs::rename(&target, &before).map_err(io)?;
            }
            moved.push(n);
            std::fs::rename(side(dir, n, "restore"), &target).map_err(io)?;
        }
        Ok(())
    })();
    if let Err(e) = swap {
        // Deshacer lo ya movido para no dejar una mezcla de datos nuevos y antiguos.
        for n in moved.iter().rev() {
            let (target, before) = (dir.join(n), side(dir, n, &tag));
            if before.exists() {
                let _ = std::fs::remove_file(&target);
                let _ = std::fs::rename(&before, &target);
            }
        }
        cleanup();
        return Err(e);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::orders;
    use crate::db::private::PrivateData;
    use crate::domain::order::{Item, OrderFilter, OrderInput};
    use crate::vault::{Vault, crypto};

    const PW: &str = "correcta-caballo-1";

    fn tmp() -> PathBuf {
        let d = std::env::temp_dir().join(format!("od-backup-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn person() -> PrivateData {
        PrivateData {
            name: Some("Persona de Prueba".into()),
            email: Some("prueba@example.invalid".into()),
            ..Default::default()
        }
    }

    /// Carpeta de datos con un pedido, sus datos privados y la sección privada abierta.
    fn populated(dir: &Path) -> (Vault, rusqlite::Connection, String) {
        let v = Vault::new(dir.to_path_buf(), crypto::FAST);
        let recovery = v.create(PW).unwrap().to_string();
        let mut c = db::open(&dir.join("business.db")).unwrap();
        let store = orders::create_store(&c, "Tienda Secreta").unwrap();
        let o = orders::create_order(
            &mut c,
            OrderInput {
                store_id: store.id,
                external_number: "#1424".into(),
                order_date: "2026-10-01".into(),
                status: None,
                shipping_cost_cents: 0,
                discount_cents: 0,
                discount_code: None,
                destination: "chile".into(),
                invoice_required: false,
                invoice_number: None,
                items: vec![Item {
                    product_id: None,
                    name: "Sensor".into(),
                    quantity: 1,
                    unit_price_cents: 1000,
                }],
            },
        )
        .unwrap();
        v.with(|p| private::set(p, &o.summary.id, person()))
            .unwrap();
        (v, c, recovery)
    }

    fn make_backup(v: &Vault, dir: &Path) -> Vec<u8> {
        let (b, p) = snapshot(dir).unwrap();
        v.with_dek(|dek| create(dek, &v.vault_bytes()?, &b, &p))
            .unwrap()
    }

    fn assert_restored(dir: &Path, password: &str) {
        let v = Vault::new(dir.to_path_buf(), crypto::FAST);
        v.unlock(password).unwrap();
        let c = db::open(&dir.join("business.db")).unwrap();
        let rows = orders::list_orders(&c, &OrderFilter::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].external_number, "#1424");
        let got = v.with(|p| private::get(p, &rows[0].id)).unwrap();
        assert_eq!(got, Some(person()));
        v.lock();
    }

    #[test]
    fn restore_in_clean_directory_with_password_and_recovery() {
        let (src, dst, dst2) = (tmp(), tmp(), tmp());
        let (v, c, recovery) = populated(&src);
        let bytes = make_backup(&v, &src);

        let (op, dek) = open(&bytes, Secret::Password(PW)).unwrap();
        restore_into(&dst, &op, &dek).unwrap();
        assert_restored(&dst, PW);

        let (op, dek) = open(&bytes, Secret::Recovery(&recovery)).unwrap();
        restore_into(&dst2, &op, &dek).unwrap();
        assert_restored(&dst2, PW);

        v.lock();
        drop(c);
        for d in [src, dst, dst2] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn backup_has_no_plain_text_and_rejects_wrong_or_tampered_input() {
        let src = tmp();
        let (v, c, _) = populated(&src);
        let bytes = make_backup(&v, &src);
        for needle in [
            "#1424",
            "Tienda Secreta",
            "Persona de Prueba",
            "SQLite format 3",
            "order_private",
        ] {
            assert!(
                !bytes.windows(needle.len()).any(|w| w == needle.as_bytes()),
                "texto en claro: {needle}"
            );
        }
        assert!(matches!(
            open(&bytes, Secret::Password("incorrecta-123")),
            Err(AppError::Auth(_))
        ));
        assert!(matches!(
            open(&bytes, Secret::Recovery("00")),
            Err(AppError::Auth(_))
        ));

        let flip = |i: usize| {
            let mut b = bytes.clone();
            b[i] ^= 1;
            b
        };
        assert!(open(&flip(bytes.len() - 1), Secret::Password(PW)).is_err()); // datos
        assert!(open(&flip(MAGIC.len() + 4 + 10), Secret::Password(PW)).is_err()); // vault.json
        assert!(open(&flip(0), Secret::Password(PW)).is_err()); // cabecera
        assert!(open(&bytes[..bytes.len() / 2], Secret::Password(PW)).is_err()); // truncada
        assert!(open(b"no soy una copia", Secret::Password(PW)).is_err());
        assert!(open(&[], Secret::Password(PW)).is_err());

        v.lock();
        drop(c);
        std::fs::remove_dir_all(src).unwrap();
    }

    #[test]
    fn restore_over_existing_data_keeps_previous_files() {
        let (src, dst) = (tmp(), tmp());
        let (v, c, _) = populated(&src);
        let bytes = make_backup(&v, &src);

        // El destino ya tiene datos distintos (otra instalación).
        let w = Vault::new(dst.clone(), crypto::FAST);
        w.create("otra-contraseña-9").unwrap();
        let other = db::open(&dst.join("business.db")).unwrap();
        orders::create_store(&other, "Tienda Antigua").unwrap();
        w.lock();
        drop(other);
        let old_business = std::fs::read(dst.join("business.db")).unwrap();

        let (op, dek) = open(&bytes, Secret::Password(PW)).unwrap();
        restore_into(&dst, &op, &dek).unwrap();
        assert_restored(&dst, PW);
        assert_eq!(
            std::fs::read(dst.join("business.db.before-restore")).unwrap(),
            old_business
        );
        assert!(dst.join("vault.json.before-restore").exists());
        assert!(
            FILES
                .iter()
                .all(|n| !dst.join(format!("{n}.restore")).exists())
        );

        v.lock();
        drop(c);
        for d in [src, dst] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn second_restore_keeps_the_first_previous_files() {
        let (src, dst) = (tmp(), tmp());
        let (v, c, _) = populated(&src);
        let bytes = make_backup(&v, &src);
        let w = Vault::new(dst.clone(), crypto::FAST);
        w.create("otra-contraseña-9").unwrap();
        w.lock();
        let original = std::fs::read(dst.join("vault.json")).unwrap();

        let (op, dek) = open(&bytes, Secret::Password(PW)).unwrap();
        restore_into(&dst, &op, &dek).unwrap();
        restore_into(&dst, &op, &dek).unwrap();
        // Los datos de antes de la primera restauración siguen intactos.
        assert_eq!(
            std::fs::read(dst.join("vault.json.before-restore")).unwrap(),
            original
        );
        assert!(dst.join("vault.json.before-restore-2").exists());

        v.lock();
        drop(c);
        for d in [src, dst] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]
    fn invalid_content_does_not_touch_existing_data() {
        // Una copia con una base de datos corrupta (pero bien cifrada) se rechaza antes de reemplazar nada.
        let (src, dst) = (tmp(), tmp());
        let (v, c, _) = populated(&src);
        let (_, p) = snapshot(&src).unwrap();
        let bytes = v
            .with_dek(|dek| create(dek, &v.vault_bytes()?, b"esto no es sqlite", &p))
            .unwrap();
        let w = Vault::new(dst.clone(), crypto::FAST);
        w.create("otra-contraseña-9").unwrap();
        w.lock();
        let before = std::fs::read(dst.join("vault.json")).unwrap();

        let (op, dek) = open(&bytes, Secret::Password(PW)).unwrap();
        assert!(restore_into(&dst, &op, &dek).is_err());
        assert_eq!(std::fs::read(dst.join("vault.json")).unwrap(), before);
        assert!(
            FILES
                .iter()
                .all(|n| !dst.join(format!("{n}.restore")).exists())
        );
        assert!(!dst.join("vault.json.before-restore").exists());

        v.lock();
        drop(c);
        for d in [src, dst] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    /// Medición de rendimiento con 1.000 pedidos (no forma parte de la suite normal):
    /// `cargo test --release perf_1000 -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn perf_1000_orders() {
        use std::time::Instant;
        let dir = tmp();
        let v = Vault::new(dir.clone(), crypto::KDF); // parámetros reales de Argon2id
        let t = Instant::now();
        v.create(PW).unwrap();
        println!("crear bóveda (Argon2id real):      {:?}", t.elapsed());

        let mut c = db::open(&dir.join("business.db")).unwrap();
        let store = orders::create_store(&c, "Tienda").unwrap();
        let t = Instant::now();
        let mut ids = vec![];
        for n in 0..1000 {
            let o = orders::create_order(
                &mut c,
                OrderInput {
                    store_id: store.id.clone(),
                    external_number: format!("#{}", 1000 + n),
                    order_date: "2026-10-01".into(),
                    status: None,
                    shipping_cost_cents: 500,
                    discount_cents: 0,
                    discount_code: None,
                    destination: if n % 3 == 0 { "foreign" } else { "chile" }.into(),
                    invoice_required: false,
                    invoice_number: None,
                    items: (0..3)
                        .map(|k| Item {
                            product_id: None,
                            name: format!("Producto {k}"),
                            quantity: 1 + k,
                            unit_price_cents: 1999,
                        })
                        .collect(),
                },
            )
            .unwrap();
            ids.push(o.summary.id);
        }
        println!("crear 1.000 pedidos:               {:?}", t.elapsed());

        let t = Instant::now();
        for id in &ids {
            v.with(|p| private::set(p, id, person())).unwrap();
        }
        println!("guardar 1.000 fichas privadas:     {:?}", t.elapsed());

        let t = Instant::now();
        let all = orders::list_orders(&c, &OrderFilter::default()).unwrap();
        println!(
            "listar 1.000 pedidos:              {:?} ({} filas)",
            t.elapsed(),
            all.len()
        );
        let t = Instant::now();
        let f = OrderFilter {
            search: Some("14".into()),
            destination: Some("chile".into()),
            ..Default::default()
        };
        let some = orders::list_orders(&c, &f).unwrap();
        println!(
            "buscar + filtrar:                  {:?} ({} filas)",
            t.elapsed(),
            some.len()
        );
        let t = Instant::now();
        orders::get_order(&c, &ids[500]).unwrap();
        println!("abrir un pedido:                   {:?}", t.elapsed());

        let t = Instant::now();
        let rows = crate::export::collect(&c, &OrderFilter::default()).unwrap();
        let json = crate::export::render(crate::export::Format::Json, &rows).unwrap();
        let csv = crate::export::render(crate::export::Format::Csv, &rows).unwrap();
        println!(
            "exportar JSON+CSV:                 {:?} ({} + {} KB)",
            t.elapsed(),
            json.len() / 1024,
            csv.len() / 1024
        );

        let t = Instant::now();
        let bytes = make_backup(&v, &dir);
        println!(
            "crear copia cifrada:               {:?} ({} KB)",
            t.elapsed(),
            bytes.len() / 1024
        );
        let t = Instant::now();
        let (op, dek) = open(&bytes, Secret::Password(PW)).unwrap();
        println!("abrir copia (Argon2id + descifrar): {:?}", t.elapsed());
        let dst = tmp();
        let t = Instant::now();
        restore_into(&dst, &op, &dek).unwrap();
        println!("restaurar (validar + reemplazar):  {:?}", t.elapsed());

        v.lock();
        drop(c);
        for d in [dir, dst] {
            std::fs::remove_dir_all(d).unwrap();
        }
    }
}
