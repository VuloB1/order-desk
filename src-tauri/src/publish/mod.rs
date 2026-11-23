//! Publicación del tablero: la app mantiene un clon propio del repositorio elegido por el usuario
//! (en su carpeta de datos) y ejecuta `git`. No guarda claves: usa las credenciales de git que ya
//! tenga el equipo. Solo se aceptan URL `https://` sin credenciales incrustadas.
use crate::error::{AppError, Result};
use std::path::Path;
use std::process::Command;

/// Valida y normaliza la dirección del repositorio (`https://…`, sin usuario ni espacios).
pub fn validate_url(url: &str) -> Result<String> {
    let url = url.trim();
    let bad = || {
        AppError::Validation("Escribe la dirección https:// del repositorio de GitHub (ej. https://github.com/usuario/pedidos)".into())
    };
    let rest = url.strip_prefix("https://").ok_or_else(bad)?;
    let host = rest.split('/').next().unwrap_or("");
    if host.is_empty()
        || host.contains('@')
        || url.len() > 300
        || url.chars().any(|c| c.is_whitespace() || c.is_control())
        || !rest.contains('/')
    {
        return Err(bad());
    }
    Ok(url.trim_end_matches('/').to_string())
}

/// Dirección pública de la página para repositorios de GitHub (`usuario.github.io/repo`).
pub fn pages_url(repo: &str) -> Option<String> {
    let rest = repo.strip_prefix("https://github.com/")?;
    let mut p = rest.trim_end_matches(".git").split('/');
    let (user, name) = (p.next()?, p.next()?);
    if user.is_empty() || name.is_empty() || p.next().is_some() {
        return None;
    }
    Some(if name.eq_ignore_ascii_case(&format!("{user}.github.io")) {
        format!("https://{}.github.io/", user.to_lowercase())
    } else {
        format!("https://{}.github.io/{name}/", user.to_lowercase())
    })
}

fn git(dir: &Path, args: &[&str]) -> Result<std::process::Output> {
    let mut cmd = Command::new("git");
    cmd.current_dir(dir)
        .args(args)
        // Sin cuadros de contraseña por consola: si faltan credenciales, falla con un aviso.
        .env("GIT_TERMINAL_PROMPT", "0");
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(&mut cmd, 0x0800_0000); // sin ventana de consola
    cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            AppError::Io("Git no está instalado (o no está en el PATH)".into())
        } else {
            AppError::Io("No se pudo ejecutar git".into())
        }
    })
}

fn run(dir: &Path, args: &[&str]) -> Result<String> {
    let out = git(dir, args)?;
    if out.status.success() {
        return Ok(String::from_utf8_lossy(&out.stdout).trim().to_string());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let last = err
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("error desconocido");
    Err(AppError::Io(format!(
        "Git falló: {}",
        last.chars().take(200).collect::<String>()
    )))
}

/// Deja `html` publicado como `index.html` del repositorio `url`, usando `dir` como clon propio.
/// Devuelve `false` si el tablero no había cambiado (nada que subir). `allow` es el protocolo
/// permitido para clonar (`https`; los tests usan `file`).
pub fn publish(dir: &Path, url: &str, html: &str, allow: &str) -> Result<bool> {
    // El clon es de la app: si cambia la dirección configurada se vuelve a clonar desde cero.
    if dir.join(".git").exists() {
        let same = run(dir, &["remote", "get-url", "origin"]).is_ok_and(|u| u == url);
        if !same {
            std::fs::remove_dir_all(dir)
                .map_err(|_| AppError::Io("No se pudo preparar la carpeta del tablero".into()))?;
        }
    }
    if !dir.join(".git").exists() {
        let parent = dir
            .parent()
            .ok_or_else(|| AppError::Io("Carpeta del tablero no válida".into()))?;
        std::fs::create_dir_all(parent)
            .map_err(|_| AppError::Io("No se pudo crear la carpeta del tablero".into()))?;
        let name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("board");
        let proto = format!("protocol.{allow}.allow=always");
        run(
            parent,
            &[
                "-c",
                "protocol.allow=never",
                "-c",
                &proto,
                "-c",
                "http.lowSpeedLimit=1000",
                "-c",
                "http.lowSpeedTime=20",
                "clone",
                "--",
                url,
                name,
            ],
        )?;
    }
    let has_commits = git(dir, &["rev-parse", "--verify", "HEAD"])?
        .status
        .success();
    if !has_commits {
        run(dir, &["checkout", "-B", "main"])?; // repositorio vacío: GitHub Pages espera `main`
    }
    std::fs::write(dir.join("index.html"), html)
        .map_err(|_| AppError::Io("No se pudo escribir el tablero".into()))?;
    if !dir.join(".nojekyll").exists() {
        std::fs::write(dir.join(".nojekyll"), "")
            .map_err(|_| AppError::Io("No se pudo escribir el tablero".into()))?;
    }
    run(dir, &["add", "index.html", ".nojekyll"])?;
    if git(dir, &["diff", "--cached", "--quiet"])?.status.success() {
        return Ok(false);
    }
    run(
        dir,
        &[
            "-c",
            "user.name=Order Desk",
            "-c",
            "user.email=order-desk@users.noreply.invalid",
            "commit",
            "-q",
            "-m",
            "Actualizar tablero",
        ],
    )?;
    run(
        dir,
        &[
            "-c",
            "http.lowSpeedLimit=1000",
            "-c",
            "http.lowSpeedTime=20",
            "push",
            "-u",
            "origin",
            "HEAD",
        ],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_https_urls_are_accepted() {
        assert_eq!(
            validate_url(" https://github.com/u/pedidos/ ").unwrap(),
            "https://github.com/u/pedidos"
        );
        for bad in [
            "",
            "github.com/u/r",
            "http://github.com/u/r",
            "ssh://git@github.com/u/r",
            "file:///c/repo",
            "https://user:pass@github.com/u/r",
            "https://github.com",
            "https://github.com/u r",
            "ext::sh -c x",
            "-oProxy=x",
        ] {
            assert!(validate_url(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn pages_address_is_derived_for_github() {
        assert_eq!(
            pages_url("https://github.com/Ana/pedidos").unwrap(),
            "https://ana.github.io/pedidos/"
        );
        assert_eq!(
            pages_url("https://github.com/ana/ana.github.io.git").unwrap(),
            "https://ana.github.io/"
        );
        assert_eq!(pages_url("https://gitlab.com/ana/pedidos"), None);
        assert_eq!(pages_url("https://github.com/ana"), None);
    }

    #[test]
    fn publishes_to_an_empty_repo_then_only_when_it_changes() {
        let base = std::env::temp_dir().join(format!("orderdesk-pub-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(&base).unwrap();
        let remote = base.join("remote.git");
        let ok = Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(&remote)
            .status()
            .unwrap()
            .success();
        assert!(ok);
        let url = remote.to_str().unwrap().to_string();
        let work = base.join("work").join("board");

        assert!(publish(&work, &url, "<p>uno</p>", "file").unwrap());
        assert!(
            !publish(&work, &url, "<p>uno</p>", "file").unwrap(),
            "sin cambios no se sube"
        );
        assert!(publish(&work, &url, "<p>dos</p>", "file").unwrap());
        let shown = Command::new("git")
            .args(["--git-dir"])
            .arg(&remote)
            .args(["show", "main:index.html"])
            .output()
            .unwrap();
        assert_eq!(String::from_utf8_lossy(&shown.stdout), "<p>dos</p>");

        // Otra dirección: se vuelve a clonar en limpio.
        let other = base.join("other.git");
        Command::new("git")
            .args(["init", "-q", "--bare"])
            .arg(&other)
            .status()
            .unwrap();
        assert!(publish(&work, other.to_str().unwrap(), "<p>tres</p>", "file").unwrap());
        let _ = std::fs::remove_dir_all(&base);
    }
}
