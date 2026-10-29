//! LUMA's opt-in mod compatibility contract. This does not alter account authentication.
use horizon_protocol::buf::{Reader, Writer};
use sha2::{Digest, Sha256};
use std::{path::Path, sync::{Arc, OnceLock}};

pub const HELLO: &str = "luma:mods_v1";
pub const PLAY: &str = "luma:orbital_v1";
pub const MARKER: &str = "LUMA_MODSET_V1";
pub const ORBITAL_SHA: &str = "5ad0f1bb284703f71334edad56c6ab58c1de1819508d44b88ca86f070007c308";

#[derive(Clone, Debug)]
pub struct Profile {
    pub fingerprint: String,
    pub shader_hashes: Arc<[String]>,
}
impl Profile {
    pub fn off() -> Self { Self { fingerprint: "none".into(), shader_hashes: Arc::from([]) } }
    pub fn enabled(&self) -> bool { self.fingerprint != "none" }
    pub fn checked(fingerprint: &str, wasm: Vec<u8>) -> Result<Self, String> {
        if !fingerprint.starts_with("orbital@0.5.3/") || fingerprint.len() > 128
            || !fingerprint.bytes().all(|c| c.is_ascii_alphanumeric() || b"@/.-".contains(&c)) {
            return Err("LUMA: unsupported mod profile".into());
        }
        let hash = format!("{:x}", Sha256::digest(&wasm));
        if hash != ORBITAL_SHA { return Err("LUMA: Orbital mod checksum mismatch".into()); }
        Ok(Self { fingerprint: fingerprint.into(), shader_hashes: Arc::from(include_str!("luma_post_hashes.txt").lines().filter(|s|!s.is_empty()).map(str::to_owned).collect::<Vec<_>>()) })
    }
    pub fn hello(&self) -> Vec<u8> {
        let mut w=Writer::new(); w.u8(1); w.string(&self.fingerprint);
        w.string(if self.enabled(){ORBITAL_SHA}else{""}); w.data
    }
    /// Shader text must come from the identical, checksum-verified component.
    /// The server never accepts arbitrary shader code from a connecting player.
    pub fn shader_allowed(&self, source: &str) -> bool {
        if !self.enabled() || source.is_empty() || source.len()>256*1024 { return false; }
        let digest=format!("{:x}",Sha256::digest(source.as_bytes()));
        self.shader_hashes.iter().any(|hash|*hash==digest)
    }
}
pub fn current() -> Result<&'static Profile, &'static str> {
    static PROFILE: OnceLock<Result<Profile,String>>=OnceLock::new();
    PROFILE.get_or_init(||{
        // Keep a capability marker in every compatible binary.
        std::hint::black_box(MARKER);
        let fingerprint=std::env::var("LUMA_MODSET").unwrap_or_else(|_|"none".into());
        if fingerprint=="none" { return Ok(Profile::off()); }
        let root=std::env::var_os("LUMA_ORBITAL_DIR").ok_or("LUMA: the Orbital package folder is missing")?;
        let path=Path::new(&root).join("mod.wasm");
        let meta=std::fs::metadata(&path).map_err(|e|format!("LUMA: cannot open Orbital: {e}"))?;
        if !meta.is_file() || meta.len()>4*1024*1024 {return Err("LUMA: invalid Orbital component".into());}
        Profile::checked(&fingerprint,std::fs::read(path).map_err(|e|e.to_string())?)
    }).as_ref().map_err(String::as_str)
}
pub fn compare(expected: &Profile, data: &[u8]) -> Result<(),String> {
    let peer=(|| -> horizon_protocol::Result<(String,String)> {
        if data.len()>256 {return Err(horizon_protocol::Error::new("LUMA hello too large"));}
        let mut r=Reader::new(data);
        if r.u8()?!=1 {return Err(horizon_protocol::Error::new("LUMA protocol version"));}
        let name=r.string(128)?;let hash=r.string(64)?;
        if !r.rest().is_empty(){return Err(horizon_protocol::Error::new("LUMA hello trailing bytes"));}
        Ok((name,hash))
    })().map_err(|_|"LUMA: неверная версия сетевого мода / invalid mod protocol".to_owned())?;
    if peer.0!=expected.fingerprint || peer.1!=if expected.enabled(){ORBITAL_SHA}else{""} {
        return Err(format!("LUMA: несовместимые моды / incompatible mods. Expected: {}; received: {}. Включите одинаковый мод и версию у всех игроков.",expected.fingerprint,peer.0));
    }
    Ok(())
}
pub fn require_peer(profile: &Profile, received: bool) -> Result<(),String> {
    if profile.enabled() && !received {Err("LUMA: другая сторона не поддерживает сетевой Orbital / the other side does not support Orbital. Установите одинаковую версию LUMA и мода.".into())}else{Ok(())}
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(name:&str)->Profile{Profile{fingerprint:name.into(),shader_hashes:Arc::from([])}}
    #[test] fn same_modes_connect_and_both_mismatch_directions_fail(){
        let off=Profile::off();let on=fixture("orbital@0.5.3/orbital-0.5.3-luma.1");
        assert!(compare(&off,&off.hello()).is_ok());assert!(compare(&on,&on.hello()).is_ok());
        assert!(compare(&on,&off.hello()).is_err());assert!(compare(&off,&on.hello()).is_err());
        assert!(compare(&on,&fixture("orbital@0.5.3/orbital-0.5.3-luma.2").hello()).is_err());
        assert!(require_peer(&on,false).is_err());assert!(require_peer(&off,false).is_ok());
    }
    #[test] fn malformed_hello_and_wrong_component_are_rejected(){
        let off=Profile::off();for bytes in [vec![],vec![255],vec![0;257]]{assert!(compare(&off,&bytes).is_err());}
        let mut extra=off.hello();extra.push(0);assert!(compare(&off,&extra).is_err());
        assert!(Profile::checked("orbital@0.5.3/orbital-0.5.3-luma.1",vec![0;8]).is_err());
    }
}
