$ErrorActionPreference = 'Continue'
Write-Host "`nNIGHTKNIGHT EXPLORE — WINDOWS DEVELOPMENT CHECK" -ForegroundColor Cyan
foreach ($name in @('git', 'gh', 'node', 'npm', 'cargo', 'rustc', 'hermes')) {
  $command = Get-Command $name -ErrorAction SilentlyContinue
  if ($command) {
    Write-Host "[FOUND] $name — $($command.Source)" -ForegroundColor Green
  } else {
    Write-Host "[MISSING] $name" -ForegroundColor Yellow
  }
}
Write-Host "`nInstall missing tools from official sources:" -ForegroundColor Gray
Write-Host '  Git: https://git-scm.com/download/win'
Write-Host '  GitHub CLI: https://cli.github.com/'
Write-Host '  Node.js LTS: https://nodejs.org/'
Write-Host '  Rust toolchain: https://rustup.rs/'
Write-Host '  Hermes: https://hermes-agent.nousresearch.com/docs/getting-started/installation'
Write-Host "`nIf already installed, open a NEW PowerShell terminal to refresh PATH."
