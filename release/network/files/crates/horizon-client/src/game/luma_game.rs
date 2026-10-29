//! LUMA's bridge between an approved local Orbital sandbox and the world host.
use super::{Game,Dim,lan::LanHost};
use horizon_client::{luma_net,luma_sync::{self,Action},luma_wire::{self,Message}};
use horizon_protocol::packets::{configuration::CustomPayload,play::{self,Clientbound as C,Serverbound as S}};
use std::{collections::HashMap,time::{Duration,Instant}};

#[derive(Default)]
pub(super) struct Client{
    pub post:Option<luma_sync::Post>,
    last:Option<Instant>,sent:Option<(u64,u64)>,sending:bool,denied:bool,
    serial:u32,pending:HashMap<u32,(u32,usize)>,
    received_posts:u64,received_sounds:u64,sent_edits:u64,
    resync_pending:bool,
}
fn heightmap(kind:u8)->horizon_mods::Heightmap{
    use horizon_mods::Heightmap::*;match kind{1=>MotionBlocking,2=>MotionBlockingNoLeaves,3=>OceanFloor,_=>WorldSurface}
}
fn heightcode(kind:horizon_mods::Heightmap)->u8{
    use horizon_mods::Heightmap::*;match kind{WorldSurface=>0,MotionBlocking=>1,MotionBlockingNoLeaves=>2,OceanFloor=>3}
}
impl Game<'_>{
    pub(super) fn luma_enabled(&self)->bool{
        (self.remote.is_some()||self.lan.is_some())&&luma_net::current().is_ok_and(|p|p.enabled())
    }
    pub(super) fn luma_reset(&mut self){self.mods.network=Client::default();if self.luma_enabled(){self.mods.network.resync_pending=true;self.luma_send(Message::Resync);}}
    pub(super) fn luma_status(&self)->serde_json::Value{
        let net=&self.mods.network;
        serde_json::json!({"enabled":luma_net::current().is_ok_and(|p|p.enabled()),"active":net.post.is_some(),"local_active":self.luma_own_active(),"post_updates":net.received_posts,"sounds_received":net.received_sounds,"edits_sent":net.sent_edits,"pending_surface":net.pending.len(),"denied":net.denied,"params":net.post.as_ref().map(|p|p.params),"data_words":net.post.as_ref().and_then(|p|p.data.as_ref()).map_or(0,|d|d.0.len())})
    }
    fn luma_own_active(&self)->bool{
        self.mods.mods.post_for("orbital").is_some_and(|p|p.params.iter().any(|v|*v!=0.))
    }
    pub(super) fn luma_sync_post(&mut self){
        if !self.luma_enabled(){return;}
        if self.mods.network.last.is_some_and(|t|t.elapsed()<Duration::from_millis(20)){return;}
        self.mods.network.last=Some(Instant::now());
        let post=self.mods.mods.post_for("orbital").filter(|p|p.params.iter().any(|v|*v!=0.));
        let Some(post)=post else{
            let was=self.mods.network.sending;self.mods.network.sending=false;self.mods.network.sent=None;self.mods.network.denied=false;
            if was{self.luma_send(Message::Clear);}return;
        };
        if self.mods.network.denied{return;}
        let data_version=post.data.as_ref().map_or(0,|(_,v)|*v);
        let known=self.mods.network.sent;
        let message=Message::Post{version:post.version,source:(!known.is_some_and(|(v,_)|v==post.version)).then_some(post.source),params:post.params,data_version,
            data:post.data.filter(|(_,v)|!known.is_some_and(|(_,old)|old==*v)).map(|(data,_)|data)};
        self.mods.network.sent=Some((post.version,data_version));self.mods.network.sending=true;self.luma_send(message);
    }
    pub(super) fn luma_render_override(&self)->bool{self.luma_enabled()&&(self.mods.network.post.is_some()||self.luma_own_active())}
    pub(super) fn luma_sound(&mut self,event:&str,at:Option<[f64;3]>,volume:f32,pitch:f32)->bool{
        if !self.luma_enabled()||!event.starts_with("orbital:")||!self.luma_own_active(){return false;}
        if !self.mods.network.denied{self.luma_send(Message::Sound{event:event.into(),at,volume,pitch});}true
    }
    pub(super) fn luma_shake(&mut self,strength:f32,seconds:f32)->bool{
        if !self.luma_enabled()||!self.luma_own_active(){return false;}
        if !self.mods.network.denied{self.luma_send(Message::Shake{strength,seconds});}true
    }
    pub(super) fn luma_post_pass(&self)->Option<super::ModPostPass>{
        self.mods.network.post.as_ref().map(|p|(p.source.clone(),p.version,p.params,p.data.clone()))
    }
    fn luma_message(&mut self,text:&str){
        self.system_message(horizon_client::text::Text::literal(text));
    }
    fn luma_send(&mut self,message:Message){
        let bytes=match message.encode(){Ok(b)=>b,Err(e)=>{self.luma_message(&e);return;}};
        if let Some(remote)=&self.remote{
            if !remote.session.send_luma(bytes) {
                self.mods.network.denied=true;
                self.luma_message("LUMA: очередь отправки Orbital переполнена / Orbital send queue is full. Reconnect and retry.");
            }
        }else if let Some(mut host)=self.lan.take(){
            if let Err(e)=self.luma_host_payload(&mut host,0,&bytes){self.mods.network.denied=true;self.luma_message(&e);}
            self.lan=Some(host);
        }
    }
    pub(super) fn luma_receive(&mut self,bytes:&[u8])->Result<(),String>{
        let profile=luma_net::current()?;
        if !profile.enabled(){return Err("LUMA: received Orbital effects with Orbital disabled".into());}
        match Message::decode(bytes)?{
            msg@Message::Post{..}=>{
                match luma_sync::Post::apply(self.mods.network.post.as_ref(),&msg,profile){
                    Ok(post)=>{self.mods.network.resync_pending=false;self.mods.network.received_posts+=1;self.mods.network.post=Some(post);},
                    Err(e) if e.contains("resynchronized")=>{self.mods.network.post=None;if !self.mods.network.resync_pending{self.mods.network.resync_pending=true;self.luma_send(Message::Resync);}},
                    Err(e)=>return Err(e),
                }
            }
            Message::Clear=>{self.mods.network.post=None;self.mods.network.resync_pending=false;}
            Message::Sound{event,at,volume,pitch}=>{self.mods.network.received_sounds+=1;self.audio.play(&event,at,volume,pitch);}
            Message::Shake{strength,seconds}=>{self.camera_shake.add(self.ticks as f64/20.,strength,seconds);}
            Message::Heights{request,heights}=>{
                if let Some((tag,n))=self.mods.network.pending.remove(&request){
                    if n!=heights.len(){return Err("LUMA: inconsistent surface-height reply".into());}
                    self.mods.mods.push_surface("orbital",horizon_mods::SurfaceHeights{tag,heights});
                }
            }
            Message::Notice{code,text}=>{
                if code==3{self.mods.network.sent=None;}else if code==1||code==2{self.mods.network.denied=true;}
                self.luma_message(&text);
            }
            _=>return Err("LUMA: unexpected server Orbital message".into()),
        }Ok(())
    }
    pub(super) fn luma_commands(&mut self,commands:Vec<String>){
        if self.mods.network.denied{return;}
        for lines in commands.chunks(32){self.luma_send(Message::Commands(lines.to_vec()));}
    }
    pub(super) fn luma_edit(&mut self,edit:horizon_mods::BlockEdit){
        if self.mods.network.denied{return;}
        self.mods.network.sent_edits+=1;
        self.mods.network.serial=self.mods.network.serial.wrapping_add(1);let transaction=self.mods.network.serial;
        self.luma_send(Message::EditStart{transaction,total:edit.runs.len(),palette:edit.palette,filters:edit.filters});
        for batch in edit.runs.chunks(luma_wire::CHUNK_RUNS){
            let runs=batch.iter().map(|r|luma_wire::Run{x:r.x,z:r.z,y:r.y,height:r.height,surface:r.surface.map(heightcode),block:r.block,only:r.only}).collect();
            self.luma_send(Message::EditRuns{transaction,runs});
        }
        self.luma_send(Message::EditEnd(transaction));
    }
    pub(super) fn luma_surface(&mut self,tag:u32,columns:Vec<(i32,i32)>,kind:horizon_mods::Heightmap){
        if self.mods.network.denied||self.mods.network.pending.len()>=32{
            self.mods.mods.push_surface("orbital",horizon_mods::SurfaceHeights{tag,heights:vec![None;columns.len()]});return;
        }
        self.mods.network.serial=self.mods.network.serial.wrapping_add(1);let request=self.mods.network.serial;
        self.mods.network.pending.insert(request,(tag,columns.len()));
        self.luma_send(Message::Surface{request,kind:heightcode(kind),columns});
    }
    fn luma_to(&mut self,host:&LanHost,conn:u32,message:&Message)->Result<(),String>{
        let bytes=message.encode()?;
        if conn==0{self.luma_receive(&bytes)}else{
            if let Some(guest)=host.hub.guests.iter().find(|g|g.conn==conn){guest.send(&C::CustomPayload(CustomPayload{channel:luma_net::PLAY.into(),data:bytes}));}Ok(())
        }
    }
    fn luma_broadcast(&mut self,host:&LanHost,dimension:&str,message:&Message)->Result<(),String>{
        let peers:Vec<u32>=host.hub.guests.iter().filter(|g|g.placed&&g.dimension==dimension).map(|g|g.conn).collect();
        for conn in peers{self.luma_to(host,conn,message)?;}
        if self.serving.is_none()&&self.remote.is_none()&&self.dim.id()==dimension{self.luma_to(host,0,message)?;}Ok(())
    }
    fn luma_broadcast_post(&mut self,host:&mut LanHost)->Result<(),String>{
        let Some(active)=&host.hub.luma.active else{return Ok(());};let dimension=active.dimension.clone();
        let mut peers:Vec<u32>=host.hub.guests.iter().filter(|g|g.placed&&g.dimension==dimension).map(|g|g.conn).collect();
        if self.serving.is_none()&&self.remote.is_none()&&self.dim.id()==dimension{peers.push(0);}
        for conn in peers{if let Some(message)=host.hub.luma.packet_for(conn){self.luma_to(host,conn,&message)?;}}Ok(())
    }
    fn luma_in_level<R>(&mut self,host:&mut LanHost,dimension:&str,f:impl FnOnce(&mut Self)->R)->Result<R,String>{
        let before=self.dim;self.enter_level(Some(host),Dim::from_id(dimension)).map_err(|e|e.to_string())?;
        let result=f(self);self.enter_level(Some(host),before).map_err(|e|e.to_string())?;Ok(result)
    }
    pub(super) fn luma_host_payload(&mut self,host:&mut LanHost,conn:u32,bytes:&[u8])->Result<(),String>{
        let profile=luma_net::current()?;host.hub.luma.budget(conn,bytes.len())?;
        let (dimension,permission)=if conn==0{(self.dim.id().to_owned(),4)}else{
            let g=host.hub.guests.iter().find(|g|g.conn==conn).ok_or("LUMA: unknown sender")?;
            (g.dimension.clone(),if g.spectator(){0}else{g.permission})
        };
        let action=host.hub.luma.receive(conn,&dimension,permission,profile,Message::decode(bytes)?)?;
        match action{
            Action::None=>{},
            Action::Updated=>self.luma_broadcast_post(host)?,
            Action::Released(dim)=>self.luma_broadcast(host,&dim,&Message::Clear)?,
            Action::Echo(message)=>self.luma_broadcast(host,&dimension,&message)?,
            Action::Reply(message)=>self.luma_to(host,conn,&message)?,
            Action::Commands(lines)=>{
                if conn==0{self.run_commands_silently(&lines);}else{for command in lines{self.lan_guest_packet(host,conn,S::ChatCommand(play::ChatCommand{command:command.trim_start_matches('/').to_owned()}));}}
            }
            Action::Edit(edit)=>{
                let edit=horizon_mods::BlockEdit{palette:edit.palette,filters:edit.filters,runs:edit.runs.into_iter().map(|r|horizon_mods::BlockRun{x:r.x,z:r.z,y:r.y,height:r.height,surface:r.surface.map(heightmap),block:r.block,only:r.only}).collect()};
                self.luma_in_level(host,&dimension,|g|g.edit_blocks_logged("orbital",&edit))?;
            }
            Action::Surface{request,kind,columns}=>{
                let heights=self.luma_in_level(host,&dimension,|g|g.surface_heights_timed(&columns,heightmap(kind)))?;
                self.luma_to(host,conn,&Message::Heights{request,heights})?;
            }
        }Ok(())
    }
    pub(super) fn luma_housekeeping(&mut self,host:&mut LanHost){
        let alive:Vec<u32>=host.hub.guests.iter().map(|g|g.conn).collect();
        if let Some(dimension)=host.hub.luma.expire(&alive){let _=self.luma_broadcast(host,&dimension,&Message::Clear);}
    }
}
