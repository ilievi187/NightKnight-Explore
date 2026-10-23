$ErrorActionPreference='Stop'
if(-not(Get-Command cargo -ErrorAction SilentlyContinue)){throw 'Rust/Cargo missing — https://rustup.rs'}
Set-Location (Join-Path $PSScriptRoot '..\crates\api')
if(-not $env:RUST_LOG){$env:RUST_LOG='info'}
cargo run
