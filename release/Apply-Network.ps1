param([Parameter(Mandatory=$true)][string]$SourceRoot)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath($SourceRoot)
$network=Join-Path $PSScriptRoot 'network'
function Get-NormalizedHash([string]$Path){
    $text=[IO.File]::ReadAllText($Path).Replace("`r`n","`n")
    $sha=[Security.Cryptography.SHA256]::Create()
    try{([BitConverter]::ToString($sha.ComputeHash([Text.Encoding]::UTF8.GetBytes($text)))).Replace('-','')}finally{$sha.Dispose()}
}
$files=Get-Content -LiteralPath (Join-Path $network 'hashes.json') -Raw|ConvertFrom-Json
foreach($file in $files){
    $path=Join-Path $root $file.path
    if($file.before -and (Get-NormalizedHash $path) -ne $file.before){throw ('Unexpected source before network patch: '+$file.path)}
    if(!$file.before -and (Test-Path -LiteralPath $path)){throw ('New network source already exists: '+$file.path)}
}
Push-Location -LiteralPath $root
try{
    & git -c core.autocrlf=false apply --check (Join-Path $network 'network.patch')
    if($LASTEXITCODE -ne 0){throw 'Network patch does not apply cleanly.'}
    & git -c core.autocrlf=false apply (Join-Path $network 'network.patch')
    if($LASTEXITCODE -ne 0){throw 'Network patch failed.'}
}finally{Pop-Location}
foreach($file in $files){
    $path=Join-Path $root $file.path
    if(!$file.before){
        [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($path))|Out-Null
        [IO.File]::Copy((Join-Path (Join-Path $network 'files') $file.path),$path)
    }
    if((Get-NormalizedHash $path) -ne $file.after){throw ('Network source checksum mismatch: '+$file.path)}
}
Write-Host 'LUMA multiplayer bridge applied; local two-client testing is still required before publication.'
