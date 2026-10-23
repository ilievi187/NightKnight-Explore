param([ValidateSet('public','private')][string]$Visibility = 'public')
$ErrorActionPreference = 'Stop'
$repo = 'ilievi187/NightKnight-Explore'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $root
foreach ($cmd in @('gh','git')) { if (-not (Get-Command $cmd -ErrorAction SilentlyContinue)) {throw "$cmd is missing. Run ./scripts/check-windows.ps1"} }
Write-Host "Publishing from: $root" -ForegroundColor Cyan
Write-Host "GitHub destination: $repo" -ForegroundColor Cyan
Write-Host "Visibility: $Visibility (GitHub repository creation/push)" -ForegroundColor Yellow
$answer=Read-Host 'Confirm creating/pushing this repository? Type YES'
if ($answer -cne 'YES') { Write-Host 'Cancelled.'; exit 1 }
& gh auth status
if ($LASTEXITCODE -ne 0) { & gh auth login; if ($LASTEXITCODE -ne 0) { throw 'GitHub login failed' } }
if (-not (Test-Path '.git')) {& git init -b main; if ($LASTEXITCODE -ne 0) {throw 'git init failed'} }
$name = (& git config user.name); $email = (& git config user.email)
if ([string]::IsNullOrWhiteSpace($name)) { & git config user.name 'NightKnight' }
if ([string]::IsNullOrWhiteSpace($email)) {
    $login = (& gh api user --jq .login).Trim()
    $id = (& gh api user --jq .id).Trim()
    if (!$login -or !$id) { throw 'Cannot determine GitHub user for commit metadata' }
    & git config user.email "$id+$login@users.noreply.github.com"
}
& git add -A
& git diff --cached --quiet
if ($LASTEXITCODE -ne 0) { & git commit -m 'chore: bootstrap NightKnight Explore FOSS MVP'; if($LASTEXITCODE -ne 0){throw 'Commit failed'} }
& gh repo view $repo --json name -q .name *> $null
$exists = ($LASTEXITCODE -eq 0)
if (-not $exists) {
    if ($Visibility -eq 'public') {
        & gh repo create $repo --public --source . --remote origin --push --description 'FOSS local-first AI-ready travel and nearby places discovery'
    } else {
        & gh repo create $repo --private --source . --remote origin --push --description 'FOSS local-first AI-ready travel and nearby places discovery'
    }
    if ($LASTEXITCODE -ne 0) {throw 'GitHub repository creation failed'}
} else {
    Write-Host 'Repository already exists. Checking remote before pushing.' -ForegroundColor Yellow
    $expected = "https://github.com/$repo.git"
    $current = (& git remote get-url origin 2>$null)
    if (-not $current) { & git remote add origin $expected }
    elseif ($current -ne $expected -and $current -ne "git@github.com:$repo.git") { throw "Existing origin points elsewhere: $current" }
    & git push -u origin main
    if ($LASTEXITCODE -ne 0) {throw 'Push failed (possibly divergent histories); inspect manually, never force push'}
}
Write-Host "SUCCESS: https://github.com/$repo" -ForegroundColor Green
