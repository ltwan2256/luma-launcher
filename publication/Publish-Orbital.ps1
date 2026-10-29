$ErrorActionPreference='Stop'
Set-StrictMode -Version 2
$receipt=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'orbital-0.5.3-luma.1.json') -Raw|ConvertFrom-Json
if($env:GITHUB_REPOSITORY -ne 'ltwan2256/luma-launcher'){throw 'Unexpected repository.'}
if(!$receipt.accepted -or $receipt.sourceCommit -notmatch '^[a-f0-9]{40}$' -or $receipt.archiveSha256 -notmatch '^[a-f0-9]{64}$'){throw 'No valid acceptance receipt.'}
foreach($check in @('twoPlayersInFrame','twoPlayersIntegrated','mismatchBothDirections','terrainShared')){
    if(!$receipt.$check){throw ('Missing acceptance check: '+$check)}
}
$tag='orbital-0.5.3-luma.1'
$release=(& gh release view $tag --repo $env:GITHUB_REPOSITORY --json isDraft,targetCommitish,tagName)|ConvertFrom-Json
if($LASTEXITCODE -ne 0 -or $release.targetCommitish -ne $receipt.sourceCommit -or $release.tagName -ne $tag){throw 'Draft source does not match the tested source.'}
$folder=Join-Path $env:RUNNER_TEMP ('luma-publish-'+[Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory($folder)|Out-Null
& gh release download $tag --repo $env:GITHUB_REPOSITORY --dir $folder --pattern orbital.json --pattern luma-orbital-0.5.3-windows-luma.1.zip
if($LASTEXITCODE -ne 0){throw 'Downloading the draft assets failed.'}
$manifest=Get-Content -LiteralPath (Join-Path $folder 'orbital.json') -Raw|ConvertFrom-Json
$archive=Join-Path $folder 'luma-orbital-0.5.3-windows-luma.1.zip'
$hash=(Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if($hash -ne $receipt.archiveSha256 -or $hash -ne $manifest.sha256 -or $manifest.releaseTag -ne $tag -or !$manifest.multiplayerReady -or !$manifest.catalogEnabled -or $manifest.bytes -ne (Get-Item -LiteralPath $archive).Length){throw 'Draft package differs from the accepted package.'}
$notes=@'
Orbital Laser 0.5.3 with the LUMA multiplayer bridge.

Download and enable it in LUMA 08 → Mods on every computer. All players need the same package and enabled state. The host opens the world to LAN and enables guest commands; friends connect to the host VPN IPv4 address and displayed port.

Shared effects, sound events and host-authorized terrain edits. One strike at a time. Changes to the checkbox require closing the game. Skins continue to synchronize through the host PC service.

Acceptance checks used two separate game processes on Windows: in-frame and integrated-server hosting, shared effects and sound messages, shared terrain edits and incompatible-mod rejection in both directions. Two physical PCs over Radmin/Hamachi were not available for this test.

Pinned Horizon revision: cbceb3f0e8dbd2eba25d3ba0c826338c93e35226. The original Stink-O Orbital component and assets are unchanged; licenses and notices are included. LUMA's native bridge sources are in this repository. Minecraft resources, worlds and account data are not distributed in this package.
'@
$noteFile=Join-Path $folder 'notes.md'
[IO.File]::WriteAllText($noteFile,$notes,(New-Object Text.UTF8Encoding($false)))
& gh release edit $tag --repo $env:GITHUB_REPOSITORY --draft=false --prerelease --notes-file $noteFile
if($LASTEXITCODE -ne 0){throw 'Publishing the accepted release failed.'}
$current=(& gh api ('repos/'+$env:GITHUB_REPOSITORY+'/contents/catalog/orbital.json'))|ConvertFrom-Json
if($LASTEXITCODE -ne 0){throw 'Reading the current catalog failed.'}
$manifest.published=$true
$text=($manifest|ConvertTo-Json -Depth 8)+"`n"
$request=@{message='Publish accepted Orbital multiplayer package in LUMA catalog';sha=$current.sha;content=[Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($text));branch='main'}
$requestFile=Join-Path $folder 'catalog-request.json'
[IO.File]::WriteAllText($requestFile,($request|ConvertTo-Json -Depth 6),(New-Object Text.UTF8Encoding($false)))
& gh api --method PUT ('repos/'+$env:GITHUB_REPOSITORY+'/contents/catalog/orbital.json') --input $requestFile --silent
if($LASTEXITCODE -ne 0){throw 'Release is published, but catalog update failed; retry the catalog update.'}
Write-Host 'Accepted Orbital package published and enabled in the LUMA catalog.'
