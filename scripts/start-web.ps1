$ErrorActionPreference='Stop'
if(-not(Get-Command npm -ErrorAction SilentlyContinue)){throw 'Node/npm missing — https://nodejs.org'}
Set-Location (Join-Path $PSScriptRoot '..\apps\web')
if(-not(Test-Path 'node_modules')){npm install --no-audit --no-fund; if($LASTEXITCODE -ne 0){throw 'npm install failed'}}
npm run dev
