param([Parameter(Mandatory=$true)][string]$SourceRoot,[Parameter(Mandatory=$true)][string]$Repository)
$ErrorActionPreference='Stop'
Set-StrictMode -Version 2
if($Repository -notmatch '^([a-zA-Z0-9-]+)/([a-zA-Z0-9_.-]+)$'){throw 'Expected owner/repository.'}
$owner=$Matches[1];$repoName=$Matches[2]
$source=[IO.Path]::GetFullPath($SourceRoot)
$buildRoot=Join-Path $env:RUNNER_TEMP 'luma-orbital-build'
[IO.Directory]::CreateDirectory($buildRoot)|Out-Null
& (Join-Path $PSScriptRoot 'Apply-Skins.ps1') -SourceRoot $source
& (Join-Path $PSScriptRoot 'Apply-Network.ps1') -SourceRoot $source
$env:CARGO_BUILD_JOBS='2'
$env:CARGO_TARGET_DIR=Join-Path $buildRoot 'target'
$contracts=Join-Path $source 'luma-contract-tests'
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'network-tests') -Destination $contracts -Recurse
Push-Location -LiteralPath $source
try {
    & cargo test --locked --manifest-path (Join-Path $contracts 'Cargo.toml')
    if($LASTEXITCODE -ne 0){throw ('Network contract tests failed: '+$LASTEXITCODE)}
    & cargo build --locked --release -p horizon-client --bin horizon-client --features mods --target x86_64-pc-windows-msvc
    if($LASTEXITCODE -ne 0){throw ('Cargo failed: '+$LASTEXITCODE)}
}finally{Pop-Location}
$kit=Join-Path $buildRoot 'kit'
[IO.Directory]::CreateDirectory((Join-Path $kit 'client'))|Out-Null
$client=Join-Path $kit 'client\horizon-client.exe'
Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc\release\horizon-client.exe') -Destination $client
Copy-Item -LiteralPath (Join-Path $source 'LICENSE') -Destination (Join-Path $kit 'LICENSE-Horizon.txt')
foreach($name in @('LICENSE-Orbital.txt','NOTICE-Orbital.txt','mod.sha256.json')){Copy-Item -LiteralPath (Join-Path $PSScriptRoot $name) -Destination (Join-Path $kit $name)}
$archive=Join-Path $buildRoot 'upstream-mod.zip'
& curl.exe --fail --location --retry 3 --connect-timeout 30 --max-time 300 --output $archive 'https://github.com/Stink-O/orbital-laser/releases/download/v0.5.3/orbital-laser-v0.5.3.zip'
if($LASTEXITCODE -ne 0){throw 'Failed to download the pinned upstream mod.'}
if((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne 'AB50BD0C1285E12D05C07381C5E6A239FBEB94A91D33197776D566DB193E42F9'){throw 'Upstream archive checksum mismatch.'}
Add-Type -AssemblyName System.IO.Compression.FileSystem
$zip=[IO.Compression.ZipFile]::OpenRead($archive)
try {
    $inventory=Get-Content -LiteralPath (Join-Path $kit 'mod.sha256.json') -Raw | ConvertFrom-Json
    foreach($file in $inventory){
        $name=[string]$file.Path
        if($name.Contains(':') -or $name -match '(^|[/\\])\.\.([/\\]|$)' -or [IO.Path]::IsPathRooted($name)){throw 'Unsafe inventory path.'}
        $entry=$zip.GetEntry('orbital-laser-v0.5.3/orbital/'+$name)
        if(!$entry -or $entry.Length -gt 128MB){throw ('Missing or oversized mod file: '+$name)}
        $destination=Join-Path (Join-Path $kit 'mod\orbital') $name
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($destination))|Out-Null
        [IO.Compression.ZipFileExtensions]::ExtractToFile($entry,$destination)
        if((Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash -ne $file.Sha256){throw ('Mod checksum mismatch: '+$name)}
    }
}finally{$zip.Dispose()}
@{Revision='cbceb3f0e8dbd2eba25d3ba0c826338c93e35226';Mod='orbital-laser-0.5.3';ClientSha256=(Get-FileHash -LiteralPath $client -Algorithm SHA256).Hash;Status='LUMA multiplayer bridge; contract tests passed';Bridge='LUMA_MODSET_V1';SourceCommit=$env:GITHUB_SHA}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $kit 'client\build-receipt.json') -Encoding UTF8
& (Join-Path $PSScriptRoot 'Prepare-Network.ps1') -KitRoot $kit -Owner $owner -Repository $repoName -ReleaseTag 'orbital-0.5.3-luma.1' -OutputRoot (Join-Path $env:GITHUB_WORKSPACE 'release-assets')
