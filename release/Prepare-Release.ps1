[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$KitRoot,
    [string]$Owner,
    [string]$Repository = 'luma-launcher',
    [ValidatePattern('^orbital-0\.5\.3-preview\.[0-9]+$')][string]$ReleaseTag = 'orbital-0.5.3-preview.1',
    [string]$OutputRoot = (Join-Path $PSScriptRoot 'prepared')
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2
Add-Type -AssemblyName System.IO.Compression.FileSystem
# Local staging only. No authentication, uploads, or changes to the installed game.
if ($Owner -and $Owner -notmatch '^[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,37}[a-zA-Z0-9])?$') { throw 'Invalid GitHub account name.' }
if ($Repository -notmatch '^[a-zA-Z0-9_.-]{1,100}$' -or $Repository -in @('.', '..')) { throw 'Invalid repository name.' }
$kit = [IO.Path]::GetFullPath($KitRoot)
$client = Join-Path $kit 'client\horizon-client.exe'
$receiptPath = Join-Path $kit 'client\build-receipt.json'
$receipt = Get-Content -LiteralPath $receiptPath -Raw | ConvertFrom-Json
$clientHash = (Get-FileHash -LiteralPath $client -Algorithm SHA256).Hash
if ($clientHash -ne $receipt.ClientSha256) { throw 'Client checksum does not match the successful build receipt.' }
$revision = 'cbceb3f0e8dbd2eba25d3ba0c826338c93e35226'
if ($receipt.Revision -ne $revision -or $receipt.Mod -ne 'orbital-laser-0.5.3') { throw 'This package expects the pinned Orbital 0.5.3 build.' }
$modRoot = [IO.Path]::GetFullPath((Join-Path $kit 'mod\orbital'))
$modPrefix = $modRoot.TrimEnd('\') + '\'
$inventory = @(Get-Content -LiteralPath (Join-Path $kit 'mod.sha256.json') -Raw | ConvertFrom-Json)
if ($inventory.Count -lt 2) { throw 'Empty mod inventory.' }
$names = New-Object 'Collections.Generic.HashSet[string]' ([StringComparer]::OrdinalIgnoreCase)
foreach ($entry in $inventory) {
    $name = [string]$entry.Path
    if ($name.Contains(':') -or $name -match '(^|[/\\])\.\.([/\\]|$)' -or !$names.Add($name.Replace('\','/'))) { throw 'Unsafe or duplicate mod inventory path.' }
    $file = [IO.Path]::GetFullPath((Join-Path $modRoot $name))
    if (!$file.StartsWith($modPrefix,[StringComparison]::OrdinalIgnoreCase)) { throw 'Mod path escapes its folder.' }
    if ((Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $entry.Sha256) { throw ('Mod checksum mismatch: ' + $name) }
}
$out = [IO.Path]::GetFullPath($OutputRoot)
[IO.Directory]::CreateDirectory($out) | Out-Null
$stage = Join-Path $out ('preview-' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($stage) | Out-Null
# Explicit allowlist: never copy resources, Minecraft jars, worlds, profiles or secrets.
[IO.Directory]::CreateDirectory((Join-Path $stage 'client')) | Out-Null
Copy-Item -LiteralPath $client -Destination (Join-Path $stage 'client\horizon-client.exe')
Copy-Item -LiteralPath $receiptPath -Destination (Join-Path $stage 'client\build-receipt.json')
foreach ($name in @('LICENSE-Horizon.txt','LICENSE-Orbital.txt','NOTICE-Orbital.txt','mod.sha256.json')) {
    Copy-Item -LiteralPath (Join-Path $kit $name) -Destination (Join-Path $stage $name)
}
foreach ($entry in $inventory) {
    $target = Join-Path (Join-Path $stage 'mod\orbital') $entry.Path
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($target)) | Out-Null
    Copy-Item -LiteralPath (Join-Path $modRoot $entry.Path) -Destination $target
}
$readme = @'
LUMA / Orbital Laser 0.5.3 - technical preview payload

This is a local staging artifact, not the finished Mods catalog release.
The existing experimental Windows client is included with the original mod.
Multiplayer synchronization and mod mismatch rejection are NOT verified or implemented by this package.
Do not advertise this payload as a multiplayer-ready release.
Minecraft resources and account data are not included.

Horizon fork revision: cbceb3f0e8dbd2eba25d3ba0c826338c93e35226
Orbital author: Stink-O, https://github.com/Stink-O/orbital-laser/releases/tag/v0.5.3
Horizon fork: https://github.com/Stink-O/horizon
Licenses and original mod notices are included.
'@
[IO.File]::WriteAllText((Join-Path $stage 'README.txt'),$readme,(New-Object Text.UTF8Encoding($false)))
$archive = Join-Path $out 'luma-orbital-0.5.3-windows-preview.zip'
if (Test-Path -LiteralPath $archive) { throw 'Output archive already exists. Choose a fresh OutputRoot; existing artifacts are never overwritten.' }
[IO.Compression.ZipFile]::CreateFromDirectory($stage,$archive,[IO.Compression.CompressionLevel]::Optimal,$false)
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
$tag = $ReleaseTag
$url = $null
if ($Owner) { $url = 'https://github.com/'+$Owner+'/'+$Repository+'/releases/download/'+$tag+'/'+[IO.Path]::GetFileName($archive) }
$manifest = [ordered]@{
    schema = 1; id = 'orbital'; name = 'Orbital Laser'; version = '0.5.3'; platform = 'windows-x64'
    releaseTag = $tag; url = $url; sha256 = $archiveHash; bytes = (Get-Item -LiteralPath $archive).Length
    published = $false; multiplayerReady = $false; catalogEnabled = $false
    clientRevision = $revision; clientSha256 = $clientHash.ToLowerInvariant()
    state = 'preview-only; requires multiplayer integration before catalog release'
}
$manifest | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $out 'orbital.preview.json') -Encoding UTF8
[IO.File]::WriteAllText((Join-Path $out 'SHA256SUMS.txt'),$archiveHash+'  '+[IO.Path]::GetFileName($archive)+[Environment]::NewLine,(New-Object Text.UTF8Encoding($false)))
Write-Host ('Prepared locally: '+$archive)
Write-Host 'Nothing was uploaded. Multiplayer release is not ready.'
