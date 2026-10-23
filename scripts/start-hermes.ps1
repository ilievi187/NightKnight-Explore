$ErrorActionPreference='Stop'
$root=(Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
if(-not(Get-Command hermes -ErrorAction SilentlyContinue)){
  Write-Host 'Hermes CLI is not in PATH.' -ForegroundColor Yellow
  Write-Host 'Official installation: https://hermes-agent.nousresearch.com/docs/getting-started/installation'
  Write-Host 'Official native Windows install (review source before executing):'
  Write-Host '  iex (irm https://hermes-agent.nousresearch.com/install.ps1)'
  throw 'Install Hermes, open a NEW terminal and run this script again.'
}
Write-Host "Hermes starting in $root" -ForegroundColor Cyan
Write-Host 'Give Hermes the instructions from HERMES-MISSION.md.' -ForegroundColor Cyan
hermes
