# Genera el instalador firmado (NSIS) y latest.json. La contraseña de la clave se pide aquí y no se guarda.
# Uso:  .\scripts\release.ps1
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$key = Join-Path $env:USERPROFILE '.tauri\order-desk.key'
if (-not (Test-Path $key)) { throw "Falta la clave de firma: $key" }

$secure = Read-Host 'Contrasena de la clave de firma' -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
$env:TAURI_SIGNING_PRIVATE_KEY = $key
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
[Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)

try {
  Set-Location $root
  & "$PSScriptRoot\cargo-env.cmd" npm run tauri build
  if ($LASTEXITCODE -ne 0) { throw 'La compilacion fallo' }
} finally {
  Remove-Item Env:TAURI_SIGNING_PRIVATE_KEY, Env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD -ErrorAction SilentlyContinue
}

$version = (Get-Content (Join-Path $root 'src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json).version
$dir = 'C:\ot\release\bundle\nsis'
$exe = "Order Desk_${version}_x64-setup.exe"
$sig = (Get-Content (Join-Path $dir "$exe.sig") -Raw).Trim()
$url = "https://github.com/VuloB1/order-desk/releases/download/v$version/" + ($exe -replace ' ', '.')
$latest = [ordered]@{
  version = $version
  notes = "Order Desk $version"
  pub_date = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
  platforms = @{ 'windows-x86_64' = @{ signature = $sig; url = $url } }
}
$out = Join-Path $dir 'latest.json'
[IO.File]::WriteAllText($out, ($latest | ConvertTo-Json -Depth 5), (New-Object Text.UTF8Encoding $false))
Write-Host "Listo. Instalador: $dir\$exe"
Write-Host "Manifiesto:      $out"
