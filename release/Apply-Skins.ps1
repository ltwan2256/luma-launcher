param([Parameter(Mandatory=$true)][string]$SourceRoot)
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath($SourceRoot)
$files=Get-Content -LiteralPath (Join-Path $PSScriptRoot 'skin-hashes.json') -Raw | ConvertFrom-Json
$pending=@()
foreach($file in $files){
    $target=Join-Path $root $file.Path
    if($file.Original){
        if((Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash -ne $file.Original){throw ('Unexpected source bytes: '+$file.Path)}
        $text=[IO.File]::ReadAllText($target)
        if($file.Path -eq 'crates/horizon-client/src/game.rs'){
            $text=$text.Replace("mod mods;`n","mod mods;`nmod luma_skins;`n")
        }else{
            $needle="        let local = r.profile;`n"
            $addition=@'
        let mut luma_players: Vec<(u128, String)> = r.tab_players.iter()
            .map(|(uuid, player)| (*uuid, player.name.clone())).collect();
        if !luma_players.iter().any(|(uuid, _)| *uuid == local) {
            luma_players.push((local, r.session.name.clone()));
        }
'@
            $text=$text.Replace($needle,$needle+$addition.Replace("`r`n","`n")+"`n")
            $needle='        resolved.retain(|(_, skin)| !skin.starts_with("skins/") || self.show_skin(skin));'
            $addition=@'
        // Local LUMA rendering override only; authentication and packets are unchanged.
        for (uuid, name) in luma_players {
            let Some(custom) = super::luma_skins::skin(&name) else { continue; };
            let location = format!("luma/{}", name.to_ascii_lowercase());
            if !self.show_skin(&location) {
                self.store_skin(location.clone(), custom.rgba.clone());
            }
            resolved.retain(|(id, _)| *id != uuid);
            resolved.push((uuid, location));
        }
'@
            $text=$text.Replace($needle,$addition.Replace("`r`n","`n")+"`n"+$needle)
        }
        $bytes=(New-Object Text.UTF8Encoding($false)).GetBytes($text)
    }else{
        if(Test-Path -LiteralPath $target){throw 'Skin module already exists; refusing to overwrite.'}
        $bytes=[IO.File]::ReadAllBytes((Join-Path $PSScriptRoot 'luma_skins.rs'))
    }
    $sha=[Security.Cryptography.SHA256]::Create()
    try{$actual=([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','')}finally{$sha.Dispose()}
    if($actual -ne $file.Patched){throw ('Patch checksum mismatch: '+$file.Path)}
    $pending+=@{Path=$target;Bytes=$bytes}
}
foreach($file in $pending){[IO.File]::WriteAllBytes($file.Path,$file.Bytes)}
Write-Host 'LUMA skin patch verified and applied.'
