//! Bounded binary messages for the Orbital bridge; never arbitrary executable payloads.
use horizon_protocol::buf::{Reader,Writer};
use std::sync::Arc;
pub const MAX_BODY:usize=1_600_000;
pub const MAX_RUNS:usize=1_048_576;
pub const CHUNK_RUNS:usize=8192;
#[derive(Clone,Debug,PartialEq)]
pub struct Run{pub x:i32,pub z:i32,pub y:i32,pub height:u32,pub surface:Option<u8>,pub block:u32,pub only:Option<u32>}
#[derive(Clone,Debug)]
pub struct Edit{pub palette:Vec<String>,pub filters:Vec<String>,pub runs:Vec<Run>}
#[derive(Clone,Debug,PartialEq)]
pub enum Message{
    Post{version:u64,source:Option<Arc<str>>,params:[f32;16],data_version:u64,data:Option<Arc<[u32]>>},
    Clear,
    Commands(Vec<String>),
    EditStart{transaction:u32,total:usize,palette:Vec<String>,filters:Vec<String>},
    EditRuns{transaction:u32,runs:Vec<Run>},
    EditEnd(u32),
    Surface{request:u32,kind:u8,columns:Vec<(i32,i32)>},
    Heights{request:u32,heights:Vec<Option<i32>>},
    Sound{event:String,at:Option<[f64;3]>,volume:f32,pitch:f32},
    Shake{strength:f32,seconds:f32},
    Notice{code:u8,text:String},
    Resync,
}
fn err()->String{"LUMA: malformed Orbital network message".into()}
fn checked(yes:bool)->Result<(),String>{if yes{Ok(())}else{Err(err())}}
fn count(r:&mut Reader<'_>,max:usize)->Result<usize,String>{let n=r.u32().map_err(|e|e.to_string())? as usize;checked(n<=max)?;Ok(n)}
fn strings(r:&mut Reader<'_>,max:usize,length:usize)->Result<Vec<String>,String>{
    let n=count(r,max)?;(0..n).map(|_|r.string(length).map_err(|e|e.to_string())).collect()
}
fn write_strings(w:&mut Writer,values:&[String]){w.u32(values.len() as u32);for v in values{w.string(v);}}
fn coordinate(v:i32)->bool{(-30_000_000..=30_000_000).contains(&v)}
impl Message{
    pub fn validate(&self)->Result<(),String>{
        match self{
            Self::Post{source,params,data,data_version,..}=>{
                checked(source.as_ref().is_none_or(|s|!s.is_empty()&&s.len()<=256*1024))?;
                checked(params.iter().all(|f|f.is_finite()))?;
                checked(data.as_ref().is_none_or(|d|d.len()<=262144))?;
                checked(*data_version>0||data.is_none())?;
            }
            Self::Commands(lines)=>checked(!lines.is_empty()&&lines.len()<=32&&lines.iter().all(|s|!s.is_empty()&&s.len()<=32767&&!s.contains(['\n','\r','\0'])))?,
            Self::EditStart{total,palette,filters,..}=>checked(*total<=MAX_RUNS&&!palette.is_empty()&&palette.len()<=4096&&filters.len()<=4096&&palette.iter().chain(filters).all(|s|!s.is_empty()&&s.len()<=256))?,
            Self::EditRuns{runs,..}=>checked(!runs.is_empty()&&runs.len()<=CHUNK_RUNS&&runs.iter().all(|r|coordinate(r.x)&&coordinate(r.z)&&(-4096..=4096).contains(&r.y)&&r.height<=4096&&r.surface.is_none_or(|v|v<4)&&r.block<4096&&r.only.is_none_or(|v|v<4096)))?,
            Self::Surface{kind,columns,..}=>checked(*kind<4&&columns.len()<=65536&&columns.iter().all(|&(x,z)|coordinate(x)&&coordinate(z)))?,
            Self::Heights{heights,..}=>checked(heights.len()<=65536&&heights.iter().all(|v|v.is_none_or(|y|(-4096..=4096).contains(&y))))?,
            Self::Sound{event,at,volume,pitch}=>checked(event.starts_with("orbital:")&&event.len()<=128&&event.bytes().all(|c|c.is_ascii_alphanumeric()||b":_./-".contains(&c))&&at.is_none_or(|v|v.iter().all(|f|f.is_finite()&&f.abs()<=30_000_000.))&&volume.is_finite()&&(0.0..=64.).contains(volume)&&pitch.is_finite()&&(0.0..=4.).contains(pitch))?,
            Self::Shake{strength,seconds}=>checked(strength.is_finite()&&(0.0..=1.).contains(strength)&&seconds.is_finite()&&(0.0..=120.).contains(seconds))?,
            Self::Notice{text,..}=>checked(text.len()<=1024)?,
            Self::Clear|Self::EditEnd(_)|Self::Resync=>{},
        }Ok(())
    }
    pub fn encode(&self)->Result<Vec<u8>,String>{
        self.validate()?;let mut w=Writer::new();
        match self{
            Self::Post{version,source,params,data_version,data}=>{
                w.u8(1);w.i64(*version as i64);w.bool(source.is_some());if let Some(s)=source{w.string(s);}
                for f in params{w.f32(*f);}w.i64(*data_version as i64);w.bool(data.is_some());
                if let Some(data)=data{w.u32(data.len() as u32);for v in data.iter(){w.u32(*v);}}
            }
            Self::Clear=>w.u8(2),
            Self::Commands(lines)=>{w.u8(3);write_strings(&mut w,lines);}
            Self::EditStart{transaction,total,palette,filters}=>{w.u8(4);w.u32(*transaction);w.u32(*total as u32);write_strings(&mut w,palette);write_strings(&mut w,filters);}
            Self::EditRuns{transaction,runs}=>{
                w.u8(5);w.u32(*transaction);w.u32(runs.len() as u32);
                for r in runs{w.i32(r.x);w.i32(r.z);w.i32(r.y);w.u32(r.height);w.u8(r.surface.unwrap_or(255));w.u32(r.block);w.i32(r.only.map_or(-1,|v|v as i32));}
            }
            Self::EditEnd(id)=>{w.u8(6);w.u32(*id);}
            Self::Surface{request,kind,columns}=>{w.u8(7);w.u32(*request);w.u8(*kind);w.u32(columns.len() as u32);for &(x,z) in columns{w.i32(x);w.i32(z);}}
            Self::Heights{request,heights}=>{w.u8(8);w.u32(*request);w.u32(heights.len() as u32);for v in heights{w.i32(v.unwrap_or(i32::MIN));}}
            Self::Sound{event,at,volume,pitch}=>{w.u8(9);w.string(event);w.bool(at.is_some());if let Some(p)=at{for v in p{w.f64(*v);}}w.f32(*volume);w.f32(*pitch);}
            Self::Shake{strength,seconds}=>{w.u8(10);w.f32(*strength);w.f32(*seconds);}
            Self::Notice{code,text}=>{w.u8(11);w.u8(*code);w.string(text);}
            Self::Resync=>w.u8(12),
        }checked(w.data.len()<=MAX_BODY)?;Ok(w.data)
    }
    pub fn decode(bytes:&[u8])->Result<Self,String>{
        checked(!bytes.is_empty()&&bytes.len()<=MAX_BODY)?;
        let mut r=Reader::new(bytes);
        let result=(||->Result<Self,horizon_protocol::Error>{Ok(match r.u8()?{
            1=>{
                let version=r.i64()? as u64;let source=if r.bool()?{Some(Arc::from(r.string(256*1024)?))}else{None};
                let mut params=[0.;16];for f in &mut params{*f=r.f32()?;}
                let data_version=r.i64()? as u64;
                let data=if r.bool()?{let n=count(&mut r,262144).map_err(horizon_protocol::Error::new)?;let mut v=Vec::with_capacity(n);for _ in 0..n{v.push(r.u32()?);}Some(Arc::from(v))}else{None};
                Self::Post{version,source,params,data_version,data}
            }
            2=>Self::Clear,
            3=>Self::Commands(strings(&mut r,32,32767).map_err(horizon_protocol::Error::new)?),
            4=>{let transaction=r.u32()?;let total=count(&mut r,MAX_RUNS).map_err(horizon_protocol::Error::new)?;Self::EditStart{transaction,total,palette:strings(&mut r,4096,256).map_err(horizon_protocol::Error::new)?,filters:strings(&mut r,4096,256).map_err(horizon_protocol::Error::new)?}},
            5=>{
                let transaction=r.u32()?;let n=count(&mut r,CHUNK_RUNS).map_err(horizon_protocol::Error::new)?;let mut runs=Vec::with_capacity(n);
                for _ in 0..n{let x=r.i32()?;let z=r.i32()?;let y=r.i32()?;let height=r.u32()?;let surface=match r.u8()?{255=>None,v=>Some(v)};let block=r.u32()?;let only=match r.i32()?{-1=>None,v=>Some(v as u32)};runs.push(Run{x,z,y,height,surface,block,only});}
                Self::EditRuns{transaction,runs}
            }
            6=>Self::EditEnd(r.u32()?),
            7=>{let request=r.u32()?;let kind=r.u8()?;let n=count(&mut r,65536).map_err(horizon_protocol::Error::new)?;let mut columns=Vec::with_capacity(n);for _ in 0..n{columns.push((r.i32()?,r.i32()?));}Self::Surface{request,kind,columns}},
            8=>{let request=r.u32()?;let n=count(&mut r,65536).map_err(horizon_protocol::Error::new)?;let mut heights=Vec::with_capacity(n);for _ in 0..n{heights.push(match r.i32()?{i32::MIN=>None,v=>Some(v)});}Self::Heights{request,heights}},
            9=>{let event=r.string(128)?;let at=if r.bool()?{Some([r.f64()?,r.f64()?,r.f64()?])}else{None};Self::Sound{event,at,volume:r.f32()?,pitch:r.f32()?}},
            10=>Self::Shake{strength:r.f32()?,seconds:r.f32()?},
            11=>Self::Notice{code:r.u8()?,text:r.string(1024)?},
            12=>Self::Resync,
            _=>return Err(horizon_protocol::Error::new("unknown LUMA message")),
        })})().map_err(|e|e.to_string())?;
        checked(r.rest().is_empty())?;result.validate()?;Ok(result)
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn bounded_messages_round_trip_and_reject_invalid_values(){
        let messages=vec![Message::Clear,Message::Commands(vec!["give @s minecraft:stone".into()]),Message::Post{version:7,source:Some(Arc::from("shader")),params:[0.;16],data_version:2,data:Some(Arc::from([0,1,u32::MAX]))},Message::Surface{request:5,kind:0,columns:vec![(1,2)]},Message::Heights{request:5,heights:vec![Some(80),None]},Message::Sound{event:"orbital:godly.charge".into(),at:Some([1.,2.,3.]),volume:1.,pitch:1.}];
        for msg in messages{assert_eq!(Message::decode(&msg.encode().unwrap()).unwrap(),msg);}
        assert!(Message::decode(&[3,255,255,255,255]).is_err());assert!(Message::decode(&vec![0;MAX_BODY+1]).is_err());assert!(Message::decode(&[2,0]).is_err());
        assert!(Message::Shake{strength:f32::NAN,seconds:1.}.encode().is_err());
        let max=Message::Post{version:1,source:Some(Arc::from("x".repeat(256*1024))),params:[0.;16],data_version:1,data:Some(Arc::from(vec![u32::MAX;262144]))};assert!(max.encode().unwrap().len()<MAX_BODY);
    }
}
