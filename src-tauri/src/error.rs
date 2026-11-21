use rusqlite::ffi;
use std::fmt;

#[derive(Debug)]
pub enum AppError {
    Validation(String),
    Duplicate(String),
    NotFound(String),
    Db(rusqlite::Error),
    /// La sección privada está bloqueada o aún no existe.
    Locked(String),
    /// Contraseña o clave de recuperación incorrecta (mensaje deliberadamente genérico).
    Auth(String),
    /// Fallo de ficheros o de cifrado, sin detalles que filtren información.
    Vault(String),
    /// Fallo al generar o guardar un fichero.
    Io(String),
}

pub type Result<T> = std::result::Result<T, AppError>;

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AppError::Validation(m)
            | AppError::Duplicate(m)
            | AppError::NotFound(m)
            | AppError::Locked(m)
            | AppError::Auth(m)
            | AppError::Vault(m)
            | AppError::Io(m) => f.write_str(m),
            AppError::Db(e) => write!(f, "Error de base de datos: {e}"),
        }
    }
}

impl std::error::Error for AppError {}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        if let rusqlite::Error::SqliteFailure(f, _) = &e {
            match f.extended_code {
                ffi::SQLITE_CONSTRAINT_UNIQUE | ffi::SQLITE_CONSTRAINT_PRIMARYKEY => {
                    return AppError::Duplicate("Ya existe un registro con ese valor".into());
                }
                ffi::SQLITE_CONSTRAINT_FOREIGNKEY => {
                    return AppError::NotFound("Referencia inexistente (tienda o producto)".into());
                }
                ffi::SQLITE_CONSTRAINT_CHECK => {
                    return AppError::Validation("Valor fuera de rango".into());
                }
                _ => {}
            }
        }
        AppError::Db(e)
    }
}

// Tauri envía el error al frontend como texto.
impl serde::Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
