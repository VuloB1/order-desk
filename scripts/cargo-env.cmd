@echo off
rem Prepara el entorno para compilar SQLCipher (OpenSSL vendored) en Windows y ejecuta el comando recibido.
rem Uso:  scripts\cargo-env.cmd npm run tauri dev     |     scripts\cargo-env.cmd cargo test --manifest-path src-tauri\Cargo.toml
set PATH=C:\Program Files (x86)\Microsoft Visual Studio\Installer;%PATH%
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul
rem Ruta de Strawberry Perl: se define en scripts\cargo-env.local.cmd (no se sube al repositorio).
if exist "%~dp0cargo-env.local.cmd" call "%~dp0cargo-env.local.cmd"
%*
