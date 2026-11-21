# Order Desk

Aplicación de escritorio para Windows, sin conexión, para gestionar pedidos comerciales (unos 200–1.000).

- **Pedidos**: número externo (`#1424`), fecha, destino (Chile / extranjero), estado con historial, productos, descuentos, envío y seguimiento.
- **Sección privada** protegida con contraseña para los datos personales de los clientes.
- **Exportar** pedidos a JSON o CSV (Excel), solo con campos públicos.
- **Copias de seguridad** cifradas y restauración.
- **Actualizaciones automáticas** desde las releases de este repositorio.

## Desarrollo
Windows con Node.js, Rust y Visual Studio Build Tools. Los comandos que compilan Rust se anteponen con `scripts\cargo-env.cmd`; crea `scripts\cargo-env.local.cmd` con el `PATH` de Strawberry Perl si hace falta.

```
npm install
scripts\cargo-env.cmd npm run tauri dev
npm test
scripts\cargo-env.cmd cargo test --manifest-path src-tauri\Cargo.toml
```

## Licencia
[MIT](LICENSE)
