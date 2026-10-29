//! Server authority, one shared Orbital strike at a time, and atomic edit assembly.
use crate::{luma_net::Profile,luma_wire::{Edit,Message,Run,MAX_RUNS}};
use std::{collections::HashMap,sync::Arc,time::{Duration,Instant}};
#[derive(Clone,Debug)]
pub struct Post {
    pub version:u64,pub source:Arc<str>,pub params:[f32;16],pub data:Option<(Arc<[u32]>,u64)>,
}
impl Post{
    pub fn apply(previous:Option<&Post>,message:&Message,profile:&Profile)->Result<Self,String>{
        let Message::Post{version,source,params,data_version,data}=message else{return Err("LUMA: expected post pass".into());};
        let source=match source{
            Some(source)=>{
                if !profile.shader_allowed(source){return Err("LUMA: shader does not belong to the approved Orbital component".into());}
                if previous.is_some_and(|p|p.version==*version&&*p.source!=**source){return Err("LUMA: conflicting shader version".into());}
                source.clone()
            }
            None=>previous.filter(|p|p.version==*version).map(|p|p.source.clone()).ok_or("LUMA: post source must be resynchronized")?,
        };
        let words=if *data_version==0{None}else if let Some(data)=data{
            if previous.and_then(|p|p.data.as_ref()).is_some_and(|(old,v)|v==data_version&&**old!=**data){return Err("LUMA: conflicting shader data version".into());}
            Some((data.clone(),*data_version))
        }else{Some(previous.and_then(|p|p.data.as_ref()).filter(|(_,v)|v==data_version).cloned().ok_or("LUMA: post data must be resynchronized")?)};
        Ok(Self{version:*version,source,params:*params,data:words})
    }
    pub fn packet(&self,known:Option<(u64,u64)>)->Message{
        let data_version=self.data.as_ref().map_or(0,|(_,v)|*v);
        Message::Post{version:self.version,source:(!known.is_some_and(|(v,_)|v==self.version)).then(||self.source.clone()),params:self.params,data_version,
            data:self.data.as_ref().filter(|(_,v)|!known.is_some_and(|(_,old)|old==*v)).map(|(data,_)|data.clone())}
    }
}
pub struct Active{pub owner:u32,pub dimension:String,pub post:Post,last:Instant,known:HashMap<u32,(u64,u64)>}
struct Pending{owner:u32,transaction:u32,total:usize,palette:Vec<String>,filters:Vec<String>,runs:Vec<Run>,last:Instant}
struct Budget{start:Instant,bytes:usize,packets:usize}
#[derive(Default)]
pub struct Server{pub active:Option<Active>,pending:Option<Pending>,budgets:HashMap<u32,Budget>}
pub enum Action{
    None,Updated,Released(String),Echo(Message),Commands(Vec<String>),Edit(Edit),Surface{request:u32,kind:u8,columns:Vec<(i32,i32)>},Reply(Message),
}
fn notice(code:u8,text:&str)->Action{Action::Reply(Message::Notice{code,text:text.into()})}
impl Server{
    pub fn budget(&mut self,conn:u32,bytes:usize)->Result<(),String>{
        let b=self.budgets.entry(conn).or_insert_with(||Budget{start:Instant::now(),bytes:0,packets:0});
        if b.start.elapsed()>=Duration::from_secs(1){b.start=Instant::now();b.bytes=0;b.packets=0;}
        b.bytes+=bytes;b.packets+=1;
        if b.bytes>32*1024*1024||b.packets>512{Err("LUMA: Orbital network rate exceeded".into())}else{Ok(())}
    }
    pub fn receive(&mut self,conn:u32,dimension:&str,permission:u8,profile:&Profile,message:Message)->Result<Action,String>{
        message.validate()?;
        if !profile.enabled(){return Err("LUMA: Orbital is disabled on this host".into());}
        if matches!(message,Message::Resync){if let Some(active)=&mut self.active{active.known.remove(&conn);}return Ok(Action::Updated);}
        if permission<2{return Ok(notice(2,"LUMA: хозяин должен разрешить команды гостям / host must allow commands for guests to use Orbital."));}
        if let Message::Clear=message{
            if self.active.as_ref().is_some_and(|a|a.owner==conn){
                let active=self.active.take().unwrap();self.pending=None;return Ok(Action::Released(active.dimension));
            }return Ok(Action::None);
        }
        if self.active.as_ref().is_some_and(|a|a.owner!=conn){
            return Ok(notice(1,"LUMA: дождитесь завершения текущего орбитального удара / wait for the current orbital strike to finish."));
        }
        if let Some(active)=&mut self.active{
            if active.dimension!=dimension{return Err("LUMA: Orbital dimension changed during a strike".into());}
            active.last=Instant::now();
        }
        Ok(match message{
            Message::Post{..}=>{
                let previous=self.active.as_ref().map(|a|&a.post);
                let post=match Post::apply(previous,&message,profile){
                    Ok(p)=>p,
                    Err(e) if e.contains("resynchronized")=>return Ok(notice(3,"LUMA: resynchronizing Orbital effects")),
                    Err(e)=>return Err(e),
                };
                if let Some(active)=&mut self.active{active.post=post;}else{
                    self.active=Some(Active{owner:conn,dimension:dimension.into(),post,last:Instant::now(),known:HashMap::new()});
                }Action::Updated
            }
            Message::Commands(commands)=>Action::Commands(commands),
            Message::EditStart{transaction,total,palette,filters}=>{
                if self.pending.is_some(){return Err("LUMA: another edit transaction is incomplete".into());}
                self.pending=Some(Pending{owner:conn,transaction,total,palette,filters,runs:Vec::new(),last:Instant::now()});Action::None
            }
            Message::EditRuns{transaction,runs}=>{
                let p=self.pending.as_mut().ok_or("LUMA: edit transaction missing")?;
                if p.owner!=conn||p.transaction!=transaction||p.runs.len()+runs.len()>p.total||p.runs.len()+runs.len()>MAX_RUNS{return Err("LUMA: edit transaction overflow".into());}
                if runs.iter().any(|r|r.block as usize>=p.palette.len()||r.only.is_some_and(|i|i as usize>=p.filters.len())){return Err("LUMA: edit palette index outside bounds".into());}
                p.runs.extend(runs);p.last=Instant::now();Action::None
            }
            Message::EditEnd(transaction)=>{
                let p=self.pending.take().ok_or("LUMA: edit transaction missing")?;
                if p.owner!=conn||p.transaction!=transaction||p.runs.len()!=p.total{return Err("LUMA: incomplete edit transaction".into());}
                Action::Edit(Edit{palette:p.palette,filters:p.filters,runs:p.runs})
            }
            Message::Surface{request,kind,columns}=>Action::Surface{request,kind,columns},
            Message::Sound{..}|Message::Shake{..}=>Action::Echo(message),
            Message::Heights{..}|Message::Notice{..}|Message::Clear|Message::Resync=>return Err("LUMA: unexpected client message".into()),
        })
    }
    pub fn packet_for(&mut self,conn:u32)->Option<Message>{
        let active=self.active.as_mut()?;
        let message=active.post.packet(active.known.get(&conn).copied());
        active.known.insert(conn,(active.post.version,active.post.data.as_ref().map_or(0,|(_,v)|*v)));
        Some(message)
    }
    pub fn expire(&mut self,alive:&[u32])->Option<String>{
        self.budgets.retain(|conn,_|*conn==0||alive.contains(conn));
        if self.pending.as_ref().is_some_and(|p|p.last.elapsed()>Duration::from_secs(30)||p.owner!=0&&!alive.contains(&p.owner)){self.pending=None;}
        if self.active.as_ref().is_some_and(|a|a.last.elapsed()>Duration::from_secs(5)||a.owner!=0&&!alive.contains(&a.owner)){
            self.pending=None;return self.active.take().map(|a|a.dimension);
        }None
    }
}

#[cfg(test)]mod tests{
    use super::*;
    fn profile()->Profile{use sha2::{Digest,Sha256};Profile{fingerprint:"orbital@test".into(),shader_hashes:Arc::from([format!("{:x}",Sha256::digest(b"approved shader"))])}}
    fn post()->Message{Message::Post{version:1,source:Some(Arc::from("approved shader")),params:[0.;16],data_version:1,data:Some(Arc::from([1,2,3]))}}
    #[test]fn shared_effects_require_permission_and_one_owner_and_resync_late_joiners(){
        let mut server=Server::default();let p=profile();
        assert!(matches!(server.receive(1,"overworld",0,&p,post()).unwrap(),Action::Reply(_)));assert!(server.active.is_none());
        assert!(matches!(server.receive(1,"overworld",2,&p,post()).unwrap(),Action::Updated));
        assert!(matches!(server.packet_for(2),Some(Message::Post{source:Some(_),data:Some(_),..})));
        assert!(matches!(server.packet_for(2),Some(Message::Post{source:None,data:None,..})));
        assert!(matches!(server.receive(2,"overworld",0,&p,Message::Resync).unwrap(),Action::Updated));
        assert!(matches!(server.packet_for(2),Some(Message::Post{source:Some(_),data:Some(_),..})));
        assert!(matches!(server.packet_for(3),Some(Message::Post{source:Some(_),data:Some(_),..})));
        assert!(matches!(server.receive(2,"overworld",2,&p,post()).unwrap(),Action::Reply(_)));
        assert!(matches!(server.receive(1,"overworld",2,&p,Message::Clear).unwrap(),Action::Released(_)));
        assert!(server.active.is_none());
    }
    #[test]fn incomplete_edits_never_apply_and_disconnect_clears_effect(){
        let mut s=Server::default();let p=profile();
        s.receive(1,"overworld",2,&p,Message::EditStart{transaction:1,total:1,palette:vec!["minecraft:air".into()],filters:vec![]}).unwrap();
        assert!(s.receive(1,"overworld",2,&p,Message::EditEnd(1)).is_err());
        s.receive(1,"overworld",2,&p,post()).unwrap();assert!(s.expire(&[]).is_some());assert!(s.active.is_none());
        let evil=Message::Post{version:2,source:Some(Arc::from("unapproved code")),params:[0.;16],data_version:0,data:None};assert!(s.receive(1,"overworld",2,&p,evil).is_err());
    }
}
