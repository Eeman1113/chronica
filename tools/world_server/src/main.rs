//! The eternal world: a headless Chronica sim that runs forever and serves a live view over
//! HTTP. The sim thread owns the world (ticking at a configured pace, autosaving so restarts
//! resume the SAME world); the server thread serves the latest rendered map + chronicle.
//!
//!   world_server [--seed N] [--port N] [--save PATH] [--days-per-min N] [--width W --height H]

use chronica_engine::inspection;
use chronica_engine::persistence;
use chronica_engine::sim::{Config, Sim};
use std::sync::{Arc, RwLock};

struct View {
    png: Vec<u8>,
    html: String,
    state: String,
}

/// The seed chain: each world's seed is derived from the last — unique, never repeating,
/// reproducible. World N's seed is splitmix^N(genesis).
fn seed_for_epoch(genesis: u64, epoch: u64) -> u64 {
    let mut s = genesis;
    for _ in 0..epoch {
        s = chronica_engine::core::rng::splitmix64(s ^ 0xE7E5_11FE_57A1_D00D);
    }
    s
}


/// The live-stream client: a canvas renderer over /state.json — wheel-zoom at the cursor,
/// drag-pan, glyph view when close, all camera state preserved across the 5-second live poll.
const LIVE_HTML: &str = r##"<!doctype html><html><head><meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>Chronica — live</title>
<style>
body{margin:0;background:#0c0a08;color:#cfc4a6;font-family:Georgia,serif;overflow:hidden}
#map{position:fixed;inset:0;cursor:grab}
#hud{position:fixed;top:0;left:0;right:0;padding:7px 14px;background:rgba(12,10,8,.9);display:flex;gap:14px;align-items:center;font-size:13px;z-index:3;flex-wrap:wrap}
#hud b{color:#e8dcbc;font-size:16px;letter-spacing:2px}
.btn{cursor:pointer;border:1px solid #4a3d30;background:#1a1510;color:#cfc4a6;padding:2px 9px;border-radius:4px;font-size:12px}
.btn.on{background:#5a4a2a;color:#ffe8b4;border-color:#8a6a3a}
.dl{color:#e8dcbc;text-decoration:none;border:1px solid #5a4a3a;padding:2px 9px;border-radius:4px}
#scrub{flex:1;min-width:180px;height:8px;background:#241d15;border-radius:4px;position:relative;cursor:pointer}
#buf{position:absolute;height:100%;background:#3a3020;border-radius:4px}
#head{position:absolute;top:-3px;width:3px;height:14px;background:#e0455a;border-radius:2px}
#side{position:fixed;top:74px;right:0;bottom:0;width:310px;background:rgba(12,10,8,.9);padding:12px 16px;overflow-y:auto;font-size:12.5px;line-height:1.5;z-index:2}
#side h2{font-size:13px;color:#b9a2d6;margin:12px 0 3px} #side ul{margin:0;padding-left:16px}
.small{color:#8d8065;font-size:12px} #side a{color:#b9a2d6}
#chron li{color:#a8b6a0}
#now{color:#e8dcbc} .behind{color:#d0a24a}
</style></head><body>
<canvas id="map"></canvas>
<div id="hud">
<b>CHRONICA</b>
<span class="btn" id="pp">❚❚</span>
<span class="btn spd" data-s="2">2</span><span class="btn spd" data-s="10">10</span>
<span class="btn spd" data-s="30">30</span><span class="btn spd" data-s="90">90</span>
<span class="btn spd" data-s="360">1yr/s</span>
<span class="btn" id="livebtn">⏭ LIVE</span>
<div id="scrub"><div id="buf"></div><div id="head"></div></div>
<span id="watch" class="small"></span>
<a class="dl" href="/download/current" download>⬇ world</a>
</div>
<div id="side"></div>
<script>
const cv=document.getElementById('map'),ctx=cv.getContext('2d');
let F=null,M=null,cells=null;
let cam={x:96,y:64,z:7};
// DVR playback state
let mode='live';           // 'live' | 'play'
let dps=30;                // playback days-per-second when playing
let playDay=0, oldest=0, live=0;
let shownDay=-1, fetching=false, lastFetch=0;
const PAL={0:['#101c38',null],1:['#0a1226',null],2:['#5a7896','═'],3:['#78280a','^'],4:['#182c58','≈'],5:['#1a2e5a','~'],6:['#969caa','∙'],7:['#46424e','▲'],8:['#343039','▒'],9:['#3c2c14','≡'],10:['#342c22','∙'],11:['#242220','"'],12:['#1a2618','♠'],13:['#18221c','↑'],14:['#1e261a','τ'],15:['#142822','"'],16:['#242416','*'],17:['#1c2818','"'],18:['#1e2618',','],19:['#262016','.']};
const FG={2:'#c8e1f5',3:'#ffaa3c',4:'#73a5eb',5:'#82b9f0',6:'#f4f6fc',7:'#f0f0f5',8:'#968aa0',9:'#e1c350',10:'#aa9678',11:'#69645f',12:'#73aa50',13:'#69875a',14:'#78965f',15:'#3c7850',16:'#8c915a',17:'#7da550',18:'#6e9655',19:'#695a41'};
const AL=['h','d','b','w','B','m','h','c','f'];
const AC=['#d2be96','#c8a06e','#966e50','#eb5a5a','#e66e3c','#e6e1d2','#be9664','#aa825a','#82b9d7'];
const CUL=['#ffe878','#78dcff','#ff96dc','#a0ffa0','#ffb478'];
const MK=['✕','‼','+','$','/'],MC=['#ff3c3c','#ff7828','#ffa0dc','#f0d25a','#c8aa78'];
function rs(){cv.width=innerWidth;cv.height=innerHeight}addEventListener('resize',rs);rs();
function w2s(x,y){return[(x-cam.x)*cam.z+cv.width/2,(y-cam.y)*cam.z+cv.height/2]}
function s2w(px,py){return[(px-cv.width/2)/cam.z+cam.x,(py-cv.height/2)/cam.z+cam.y]}
cv.addEventListener('wheel',e=>{e.preventDefault();const[wx,wy]=s2w(e.clientX,e.clientY);
 cam.z=Math.min(48,Math.max(2.5,cam.z*(e.deltaY<0?1.12:0.89)));
 const[nx,ny]=s2w(e.clientX,e.clientY);cam.x+=wx-nx;cam.y+=wy-ny;},{passive:false});
let drag=null;
cv.addEventListener('mousedown',e=>{drag=[e.clientX,e.clientY];cv.style.cursor='grabbing'});
addEventListener('mouseup',()=>{drag=null;cv.style.cursor='grab'});
addEventListener('mousemove',e=>{if(drag){cam.x-=(e.clientX-drag[0])/cam.z;cam.y-=(e.clientY-drag[1])/cam.z;drag=[e.clientX,e.clientY]}});
cv.addEventListener('touchstart',e=>{if(e.touches.length==1)drag=[e.touches[0].clientX,e.touches[0].clientY]},{passive:true});
cv.addEventListener('touchmove',e=>{if(drag&&e.touches.length==1){const t=e.touches[0];cam.x-=(t.clientX-drag[0])/cam.z;cam.y-=(t.clientY-drag[1])/cam.z;drag=[t.clientX,t.clientY]}},{passive:true});
// controls
document.getElementById('pp').onclick=()=>{mode=(mode==='pause')?'play':'pause';syncbtn()};
document.getElementById('livebtn').onclick=()=>{mode='live';syncbtn()};
document.querySelectorAll('.spd').forEach(b=>b.onclick=()=>{dps=+b.dataset.s;mode='play';if(playDay<oldest)playDay=oldest;syncbtn()});
document.getElementById('scrub').onclick=e=>{const r=e.currentTarget.getBoundingClientRect();
 const f=(e.clientX-r.left)/r.width;playDay=oldest+f*(live-oldest);mode='play';syncbtn()};
function syncbtn(){document.getElementById('pp').textContent=mode==='pause'?'▶':'❚❚';
 document.getElementById('livebtn').classList.toggle('on',mode==='live');
 document.querySelectorAll('.spd').forEach(b=>b.classList.toggle('on',mode==='play'&&+b.dataset.s===dps));}
async function pollMeta(){try{const r=await fetch('/live.json',{cache:'no-store'});M=await r.json();
 oldest=M.oldestDay;live=M.liveDay;if(mode==='live'||playDay>live)playDay=live;if(playDay<oldest)playDay=oldest;
 sidebar();}catch(e){}setTimeout(pollMeta,2000)}
async function fetchFrame(day){if(fetching)return;const now=performance.now();if(now-lastFetch<70)return;
 fetching=true;lastFetch=now;
 try{const r=await fetch('/frame?day='+Math.round(day),{cache:'no-store'});const f=await r.json();
  if(f&&f.cells){F=f;const bin=atob(f.cells);cells=new Uint8Array(bin.length);for(let i=0;i<bin.length;i++)cells[i]=bin.charCodeAt(i);shownDay=f.day}}catch(e){}
 fetching=false}
let prevT=performance.now();
function loop(t){requestAnimationFrame(loop);const dt=(t-prevT)/1000;prevT=t;
 if(mode==='live'){playDay=live}
 else if(mode==='play'){playDay+=dps*dt;if(playDay>=live){playDay=live;mode='live';syncbtn()}if(playDay<oldest)playDay=oldest}
 // fetch the frame nearest the play head when it moves off the shown one
 if(Math.abs(playDay-shownDay)>=2)fetchFrame(playDay);
 draw();hud();}
function hud(){const w=document.getElementById('watch');if(!M)return;
 const wy=Math.floor(playDay/360),ly=Math.floor(live/360),beh=ly-wy;
 const bufpct=live>oldest?((playDay-oldest)/(live-oldest)*100):100;
 document.getElementById('buf').style.width='100%';
 document.getElementById('head').style.left=bufpct+'%';
 w.innerHTML='watching <span id="now">Year '+wy+'</span> · live Year '+ly+(beh>0?' <span class="behind">(−'+beh+'y)</span>':' <span class="on" style="color:#e0455a">●LIVE</span>');}
function draw(){if(!F||!cells)return;
 ctx.fillStyle='#0c0a08';ctx.fillRect(0,0,cv.width,cv.height);
 const z=cam.z,W=F.w,H=F.h,ascii=z>=12;
 const x0=Math.max(0,Math.floor(cam.x-cv.width/2/z)-1),x1=Math.min(W-1,Math.ceil(cam.x+cv.width/2/z)+1);
 const y0=Math.max(0,Math.floor(cam.y-cv.height/2/z)-1),y1=Math.min(H-1,Math.ceil(cam.y+cv.height/2/z)+1);
 const season=Math.floor((F.day%360)/90);
 for(let y=y0;y<=y1;y++)for(let x=x0;x<=x1;x++){const b=cells[y*W+x],c=b&63;
  const[sx,sy]=w2s(x,y);const p=PAL[c]||PAL[19];
  ctx.fillStyle=p[0];ctx.fillRect(sx,sy,z+1,z+1);
  if(ascii&&p[1]){ctx.fillStyle=(c==9&&[['#96783c','#bebe46','#e6c350','#8c7d5f'][season]])||FG[c]||'#888';
   ctx.font=Math.round(z*0.9)+'px monospace';ctx.textAlign='center';ctx.textBaseline='middle';ctx.fillText(p[1],sx+z/2,sy+z/2);}}
 for(const b of F.buildings){if(b.x<x0||b.x>x1||b.y<y0||b.y>y1)continue;const[sx,sy]=w2s(b.x,b.y);
  const g=!b.r?['□','#828282']:b.p<100?['□','#c8aa6e']:[['⌂','#d2a05a'],['▦','#e6c878'],['†','#d2bea0'],['#','#bea578']][b.k]||['⌂','#d2a05a'];
  if(ascii){ctx.fillStyle=g[1];ctx.font=Math.round(z*0.9)+'px monospace';ctx.fillText(g[0],sx+z/2,sy+z/2);
   if(z>=14&&b.s>0){ctx.fillStyle='#ffe8b4';ctx.font=Math.round(z*0.38)+'px monospace';ctx.fillText(b.s,sx+z/2,sy+z*1.1)}}
  else{ctx.fillStyle='#a87c46';ctx.fillRect(sx+z*0.1,sy+z*0.1,z*0.8,z*0.8)}}
 for(const a of F.animals){if(a.x<x0||a.x>x1||a.y<y0||a.y>y1)continue;const[sx,sy]=w2s(a.x,a.y);
  if(ascii){ctx.fillStyle=AC[a.s]||'#ccc';ctx.font=Math.round(z*0.9)+'px monospace';ctx.fillText(AL[a.s]||'?',sx+z/2,sy+z/2);
   if(z>=22){ctx.fillStyle='rgba(255,255,255,.75)';ctx.font=Math.round(z*0.3)+'px Georgia';ctx.fillText(a.a,sx+z/2,sy-z*0.15)}}
  else{ctx.fillStyle=(a.s==3||a.s==4)?'#e15050':'#cdaf84';ctx.beginPath();ctx.arc(sx+z/2,sy+z/2,Math.max(1.4,z*0.28),0,7);ctx.fill()}}
 for(const p of F.people){if(p.x<x0||p.x>x1||p.y<y0||p.y>y1)continue;const[sx,sy]=w2s(p.x,p.y);
  if(ascii){ctx.fillStyle=CUL[p.c%5];ctx.font=Math.round(z*0.9)+'px monospace';ctx.fillText(p.k?'•':'☺',sx+z/2,sy+z/2);
   if(z>=20){ctx.fillStyle='#fff';ctx.font=Math.round(z*0.34)+'px Georgia';ctx.fillText(p.n+' — '+p.a,sx+z/2,sy-z*0.2)}}
  else{ctx.fillStyle='#fcf06e';ctx.beginPath();ctx.arc(sx+z/2,sy+z/2,Math.max(1.8,z*0.34),0,7);ctx.fill()}}
 for(const m of F.marks){if(m.x<x0||m.x>x1||m.y<y0||m.y>y1)continue;const[sx,sy]=w2s(m.x,m.y);
  ctx.fillStyle=MC[m.t];ctx.font=Math.max(10,z*0.7)+'px monospace';ctx.textAlign='center';ctx.fillText(MK[m.t],sx+z*0.8,sy+z*0.2)}
 ctx.textAlign='center';for(const st of F.setts){const[sx,sy]=w2s(st.x,st.y-2);ctx.fillStyle='#fff';ctx.font='13px Georgia';ctx.fillText(st.n,sx,sy)}
}
function sidebar(){if(!M)return;const sd=document.getElementById('side');let h='';
 h+='<div class="small">the world lives at full speed; you watch at yours</div>';
 h+='<h2>The world now — '+M.date+'</h2><ul><li><b>People: '+M.stats.people+'</b></li>';
 for(const s of M.stats.species)if(s.c>0)h+='<li>'+s.n+': '+s.c+'</li>';
 h+='<li>Plants: '+M.stats.plants+' ('+M.stats.trees+' trees, '+Math.round(M.stats.cover*100)+'% forest)</li></ul>';
 const ex=M.stats.species.filter(s=>s.c==0).map(s=>s.n);
 if(ex.length)h+='<div class="small">gone: '+ex.join(', ')+'</div>';
 const fs=M.faiths.filter(f=>f.c>0),fg=M.faiths.length-fs.length;
 h+='<h2>Faiths</h2><ul>'+fs.map(f=>'<li>'+f.n+': '+f.c+'</li>').join('')+(fg?'<li class="small">…and '+fg+' whose last believer is gone</li>':'')+'</ul>';
 h+='<h2>The chronicle</h2><ul id="chron">'+M.chron.map(c=>'<li>'+c+'</li>').join('')+'</ul>';
 h+='<div class="small">events since this world began: '+M.stats.events+'</div>';
 if(M.past.length){h+='<h2>Worlds that were</h2><ul>'+M.past.map(p=>'<li>World '+p.no+' · yr '+p.year+' · '+p.events+' events<br><a href="/archive/'+p.base+'.png">portrait</a> · <a href="/archive/'+p.base+'.crn" download>⬇ save</a></li>').join('')+'</ul>'}
 sd.innerHTML=h}
syncbtn();pollMeta();requestAnimationFrame(loop);
</script></body></html>"##;

fn main() {
    let mut seed = 1u64;
    let mut port = 80u16;
    let mut save = String::from("/var/lib/chronica/world.crn");
    let mut days_per_min = 0u64; // 0 = as fast as the engine runs; the DVR decouples watching
    let mut width = 192u32;
    let mut height = 128u32;
    let mut it = std::env::args().skip(1);
    while let Some(k) = it.next() {
        let mut v = || it.next().expect("value");
        match k.as_str() {
            "--seed" => seed = v().parse().unwrap(),
            "--port" => port = v().parse().unwrap(),
            "--save" => save = v(),
            "--days-per-min" => days_per_min = v().parse().unwrap(),
            "--width" => width = v().parse().unwrap(),
            "--height" => height = v().parse().unwrap(),
            _ => {}
        }
    }
    let data_dir = std::path::Path::new(&save).parent().unwrap().to_path_buf();
    std::fs::create_dir_all(&data_dir).ok();
    let archive_dir = data_dir.join("archive");
    std::fs::create_dir_all(&archive_dir).ok();
    let registry_path = data_dir.join("worlds.log");
    let genesis = seed;

    // which epoch are we in? the registry of dead worlds remembers
    let mut epoch: u64 = std::fs::read_to_string(&registry_path)
        .map(|t| t.lines().count() as u64)
        .unwrap_or(0);

    // resume the same world if it exists — the whole point is continuity
    let save_path = std::path::PathBuf::from(&save);
    let mut sim = if save_path.exists() {
        eprintln!("resuming world (epoch {epoch}) from {save}");
        persistence::load_from_file(&save_path).expect("load save")
    } else {
        let s = seed_for_epoch(genesis, epoch);
        eprintln!("creating world {} (seed {s})", epoch + 1);
        Sim::new(Config { seed: s, width, height })
    };
    // when did the last creature die? (u64::MAX = life persists)
    let mut doomsday: u64 = u64::MAX;

    let view = Arc::new(RwLock::new(View { png: Vec::new(), html: String::new(), state: String::new() }));
    {
        let mut v = view.write().unwrap();
        *v = render_view(&sim);
    }
    // the current world's full binary (save + entire event history), refreshed each autosave
    let save_bytes: Arc<RwLock<Vec<u8>>> = Arc::new(RwLock::new(sim.to_bytes()));
    let cur_seed: Arc<RwLock<u64>> = Arc::new(RwLock::new(sim.cfg.seed));
    // the DVR: a rolling buffer of world-frames the client can replay at its own pace
    use std::collections::VecDeque;
    const FRAME_STRIDE: u64 = 2; // capture a frame every N sim-days
    const MAX_FRAMES: usize = 2400; // ~4800 sim-days of rewind (~13 years)
    let frames: Arc<RwLock<VecDeque<(u64, String)>>> = Arc::new(RwLock::new(VecDeque::new()));
    frames.write().unwrap().push_back((sim.clock.day, render_frame(&sim)));
    let meta: Arc<RwLock<String>> = Arc::new(RwLock::new(render_meta(&sim, sim.clock.day, sim.clock.day)));

    // ---- HTTP server thread ----
    let server_view = Arc::clone(&view);
    let server_save = Arc::clone(&save_bytes);
    let server_seed = Arc::clone(&cur_seed);
    let server_frames = Arc::clone(&frames);
    let server_meta = Arc::clone(&meta);
    std::thread::spawn(move || {
        let server = tiny_http::Server::http(("0.0.0.0", port)).expect("bind http");
        eprintln!("serving on port {port}");
        for req in server.incoming_requests() {
            let url = req.url().to_string();
            let v = server_view.read().unwrap();
            let resp = if url.starts_with("/archive/") {
                // the museum of dead worlds
                let name = url.trim_start_matches("/archive/");
                let safe = !name.contains("..") && !name.contains('/');
                let path = std::path::Path::new("/var/lib/chronica/archive").join(name);
                if safe && path.exists() {
                    let bytes = std::fs::read(&path).unwrap_or_default();
                    let ct: &[u8] = if name.ends_with(".png") {
                        b"image/png"
                    } else if name.ends_with(".html") {
                        b"text/html; charset=utf-8"
                    } else {
                        b"application/octet-stream"
                    };
                    let mut r = tiny_http::Response::from_data(bytes).with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], ct).unwrap(),
                    );
                    if name.ends_with(".crn") {
                        r.add_header(
                            tiny_http::Header::from_bytes(
                                &b"Content-Disposition"[..],
                                format!("attachment; filename=\"{name}\"").as_bytes(),
                            )
                            .unwrap(),
                        );
                    }
                    r
                } else {
                    tiny_http::Response::from_data(b"gone".to_vec())
                }
            } else if url.starts_with("/download/current") {
                // the living world, whole: state + full causal history, one file
                let bytes = server_save.read().unwrap().clone();
                let seed = *server_seed.read().unwrap();
                let fname = format!("chronica_world_seed_{seed:016x}.crn");
                tiny_http::Response::from_data(bytes)
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Type"[..],
                            &b"application/octet-stream"[..],
                        )
                        .unwrap(),
                    )
                    .with_header(
                        tiny_http::Header::from_bytes(
                            &b"Content-Disposition"[..],
                            format!("attachment; filename=\"{fname}\"").as_bytes(),
                        )
                        .unwrap(),
                    )
            } else if url.starts_with("/frame") {
                // the nearest recorded frame to ?day=D (default: newest)
                let want: Option<u64> = url
                    .split_once("day=")
                    .and_then(|(_, r)| r.split(|c: char| !c.is_ascii_digit()).next())
                    .and_then(|d| d.parse().ok());
                let fr = server_frames.read().unwrap();
                let body = if let Some(d) = want {
                    fr.iter()
                        .min_by_key(|(fd, _)| (*fd as i64 - d as i64).abs())
                        .map(|(_, j)| j.clone())
                } else {
                    fr.back().map(|(_, j)| j.clone())
                }
                .unwrap_or_else(|| "{}".to_string());
                tiny_http::Response::from_data(body.into_bytes()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                        .unwrap(),
                )
            } else if url.starts_with("/live.json") {
                tiny_http::Response::from_data(server_meta.read().unwrap().clone().into_bytes())
                    .with_header(
                        tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..])
                            .unwrap(),
                    )
            } else if url.starts_with("/classic") {
                tiny_http::Response::from_data(v.html.clone().into_bytes()).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap(),
                )
            } else if url.starts_with("/map.png") {
                tiny_http::Response::from_data(v.png.clone()).with_header(
                    tiny_http::Header::from_bytes(&b"Content-Type"[..], &b"image/png"[..])
                        .unwrap(),
                )
            } else {
                tiny_http::Response::from_data(LIVE_HTML.as_bytes().to_vec()).with_header(
                    tiny_http::Header::from_bytes(
                        &b"Content-Type"[..],
                        &b"text/html; charset=utf-8"[..],
                    )
                    .unwrap(),
                )
            };
            let _ = req.respond(resp);
        }
    });

    // ---- the worlds, forever: each runs until a century after its last creature ----
    // days_per_min == 0 → run as fast as the engine can; the DVR decouples watching from living
    let tick_sleep = if days_per_min == 0 {
        std::time::Duration::ZERO
    } else {
        std::time::Duration::from_millis(60_000 / days_per_min.max(1))
    };
    let mut last_frame_day = sim.clock.day;
    let mut last_save = std::time::Instant::now();
    let mut last_meta = std::time::Instant::now();
    loop {
        let t0 = std::time::Instant::now();
        sim.tick();

        // capture a frame into the DVR buffer every few sim-days
        if sim.clock.day.saturating_sub(last_frame_day) >= FRAME_STRIDE {
            last_frame_day = sim.clock.day;
            let f = render_frame(&sim);
            let mut fr = frames.write().unwrap();
            fr.push_back((sim.clock.day, f));
            while fr.len() > MAX_FRAMES {
                fr.pop_front();
            }
        }
        // refresh the live sidebar a few times a second (real time), not per sim-tick
        if last_meta.elapsed() >= std::time::Duration::from_millis(500) {
            last_meta = std::time::Instant::now();
            let oldest = frames.read().unwrap().front().map(|(d, _)| *d).unwrap_or(sim.clock.day);
            *meta.write().unwrap() = render_meta(&sim, oldest, sim.clock.day);
        }

        // watch for the death of the last creature (plants alone don't count)
        let creatures = chronica_engine::humans::population(&sim)
            + sim.animals.list.iter().filter(|a| a.alive).count();
        if creatures == 0 {
            if doomsday == u64::MAX {
                doomsday = sim.clock.day;
                eprintln!(
                    "[{}] the last creature has died — the plants inherit the world for 100 years",
                    sim.clock.date_string()
                );
            }
        } else {
            doomsday = u64::MAX;
        }

        // a century of silence, then the archive and a new genesis
        if doomsday != u64::MAX && sim.clock.day >= doomsday + 100 * 360 {
            let world_no = epoch + 1;
            let this_seed = sim.cfg.seed;
            let year = sim.clock.day / 360;
            eprintln!("world {world_no} ends in year {year}; archiving…");
            let base = format!("world_{world_no:03}_seed_{this_seed:016x}_year_{year}");
            let _ = persistence::save_to_file(&sim, &archive_dir.join(format!("{base}.crn")));
            let _ = std::fs::write(archive_dir.join(format!("{base}.png")), render_png(&sim));
            let _ = std::fs::write(archive_dir.join(format!("{base}.html")), render_html(&sim));
            // one line per world, forever
            let line = format!(
                "{world_no}	{this_seed:016x}	{year}	{}	{}
",
                sim.history.events.len(),
                base
            );
            use std::io::Write as _;
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&registry_path)
            {
                let _ = f.write_all(line.as_bytes());
            }
            let _ = std::fs::remove_file(&save_path);
            epoch += 1;
            let s = seed_for_epoch(genesis, epoch);
            eprintln!("world {} begins (seed {s})", epoch + 1);
            sim = Sim::new(Config { seed: s, width, height });
            doomsday = u64::MAX;
            *save_bytes.write().unwrap() = sim.to_bytes();
            *cur_seed.write().unwrap() = sim.cfg.seed;
            {
                let mut fr = frames.write().unwrap();
                fr.clear();
                fr.push_back((sim.clock.day, render_frame(&sim)));
            }
            last_frame_day = sim.clock.day;
            *meta.write().unwrap() = render_meta(&sim, sim.clock.day, sim.clock.day);
            *view.write().unwrap() = render_view(&sim);
            continue;
        }
        // autosave by real time (the sim may be doing thousands of days a second)
        if last_save.elapsed() >= std::time::Duration::from_secs(20) {
            last_save = std::time::Instant::now();
            let tmp = save_path.with_extension("crn.tmp");
            if persistence::save_to_file(&sim, &tmp).is_ok() {
                let _ = std::fs::rename(&tmp, &save_path);
                *save_bytes.write().unwrap() = sim.to_bytes();
                eprintln!("[{}] autosaved", sim.clock.date_string());
            }
        }
        if tick_sleep > std::time::Duration::ZERO {
            let spent = t0.elapsed();
            if spent < tick_sleep {
                std::thread::sleep(tick_sleep - spent);
            }
        }
    }
}

fn render_view(sim: &Sim) -> View {
    View { png: render_png(sim), html: render_html(sim), state: String::new() }
}

/// One cell, classified for the client renderer (mirror of the desktop glyph logic).
fn classify_cell(sim: &Sim, i: usize) -> (u8, u8) {
    let g = &sim.grid;
    let shade = ((chronica_engine::core::rng::splitmix64(i as u64) >> 32) % 4) as u8;
    if g.ocean[i] {
        return (if g.elev[i] < -0.45 { 1 } else { 0 }, shade);
    }
    if g.ice_bears(i) {
        return (2, shade);
    }
    if g.burning[i] > 0 {
        return (3, shade);
    }
    if g.surface[i] > 0.12 {
        return (4, shade); // flood/lake
    }
    if g.is_river(i) {
        return (5, shade);
    }
    if g.snow[i] > 0.02 {
        return (6, shade);
    }
    if g.elev[i] > 1.02 {
        return (7, shade);
    }
    if g.elev[i] > 0.82 {
        return (8, shade);
    }
    if g.crop_cover.get(i).copied().unwrap_or(0.0) > 0.05 {
        return (9, shade); // field
    }
    if g.is_path(i) {
        return (10, shade);
    }
    if g.burn_scar[i] > 0.3 {
        return (11, shade);
    }
    // dominant plant
    let (mut oak, mut pine, mut reed, mut shrub, mut grass) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    if let Some(pids) = sim.plants.by_cell.get(i) {
        for &pi in pids {
            let pl = &sim.plants.list[pi as usize];
            if !pl.alive {
                continue;
            }
            match pl.species {
                chronica_engine::species::SP_OAK => oak += pl.biomass,
                chronica_engine::species::SP_PINE => pine += pl.biomass,
                chronica_engine::species::SP_REED => reed += pl.biomass,
                chronica_engine::species::SP_SCRUB => shrub += pl.biomass,
                chronica_engine::species::SP_WHEAT => {}
                _ => grass += pl.biomass,
            }
        }
    }
    if oak > 1.2 || pine > 1.2 {
        return (if pine > oak { 13 } else { 12 }, shade);
    }
    if oak + pine > 0.25 {
        return (14, shade);
    }
    if reed > 0.15 {
        return (15, shade);
    }
    if shrub > 0.25 {
        return (16, shade);
    }
    if grass > 0.5 {
        return (17, shade);
    }
    if grass > 0.10 {
        return (18, shade);
    }
    (19, shade)
}

fn render_frame(sim: &Sim) -> String {
    use base64::Engine as _;
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let g = &sim.grid;
    let n = g.n();
    let mut cells = Vec::with_capacity(n);
    for i in 0..n {
        let (c, sh) = classify_cell(sim, i);
        cells.push(c | (sh << 6));
    }
    let cells_b64 = base64::engine::general_purpose::STANDARD.encode(&cells);

    let day = sim.clock.day;
    let people: Vec<serde_json::Value> = sim
        .humans
        .list
        .iter()
        .filter(|h| h.alive)
        .map(|h| {
            let doing = match h.current {
                chronica_engine::humans::HumanAction::Gather => "gathering",
                chronica_engine::humans::HumanAction::Drink { .. } => "going to water",
                chronica_engine::humans::HumanAction::EatStored => "eating",
                chronica_engine::humans::HumanAction::Hunt { .. } => "hunting!",
                chronica_engine::humans::HumanAction::ChopWood => "chopping wood",
                chronica_engine::humans::HumanAction::Build { .. } => "building",
                chronica_engine::humans::HumanAction::Deposit => "storing food",
                chronica_engine::humans::HumanAction::Socialize { .. } => "talking",
                chronica_engine::humans::HumanAction::Court { .. } => "courting",
                chronica_engine::humans::HumanAction::TendFarm => "farming",
                chronica_engine::humans::HumanAction::PreserveFood => "smoking food",
                chronica_engine::humans::HumanAction::Rest => "resting",
                chronica_engine::humans::HumanAction::Flee { .. } => "fleeing!",
                chronica_engine::humans::HumanAction::MoveTo { .. } => "walking",
                chronica_engine::humans::HumanAction::Idle => "idling",
            };
            let child = (day as i64 - h.born) < 14 * 360;
            serde_json::json!({"x":h.x,"y":h.y,"n":h.name,"c":h.culture,"a":doing,"k":child})
        })
        .collect();
    let animals: Vec<serde_json::Value> = sim
        .animals
        .list
        .iter()
        .filter(|a| a.alive)
        .map(|a| {
            let doing = match a.rationale.chosen {
                0 => "fleeing!",
                1 => "to water",
                2 => "grazing",
                3 => "hunting!",
                4 => "courting",
                5 => "with herd",
                6 => "resting",
                _ => "roaming",
            };
            serde_json::json!({"x":a.x,"y":a.y,"s":a.species,"a":doing,"t":!a.tamed_by.is_none()})
        })
        .collect();
    let buildings: Vec<serde_json::Value> = sim
        .objects
        .buildings
        .iter()
        .filter(|b| b.exists || b.progress >= 1.0)
        .map(|b| {
            let (x, y) = g.xy(b.cell as usize);
            let k = match b.kind {
                chronica_engine::objects::BuildingKind::Hut => 0,
                chronica_engine::objects::BuildingKind::Granary => 1,
                chronica_engine::objects::BuildingKind::Hall => 2,
                chronica_engine::objects::BuildingKind::Palisade => 3,
            };
            serde_json::json!({"x":x,"y":y,"k":k,"p":(b.progress*100.0) as u32,
                "r":b.exists,"s":(b.food_store+b.preserved_store) as u32})
        })
        .collect();
    let setts: Vec<serde_json::Value> = chronica_engine::society::living_settlements(sim)
        .into_iter()
        .map(|(nm, (x, y))| serde_json::json!({"n":nm,"x":x,"y":y}))
        .collect();
    // recent marks
    let mut marks = Vec::new();
    for ev in sim.history.events.iter().rev().take(600) {
        if day.saturating_sub(ev.day) > 3 {
            break;
        }
        let Some(loc) = ev.loc else { continue };
        let (x, y) = g.xy(loc as usize);
        let t = match &ev.kind {
            EK::Killed { .. } => 0,
            EK::RaidCarriedOut { .. } => 1,
            EK::Born { .. } => 2,
            EK::HarvestedFood { .. } => 3,
            EK::FelledTree { .. } => 4,
            _ => continue,
        };
        marks.push(serde_json::json!({"x":x,"y":y,"t":t}));
    }
    // stats + chronicle + faiths + past worlds (client renders panels)
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = Vec::new();
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push(inspection::describe(sim, ev));
            if chron.len() >= 30 {
                break;
            }
        }
    }
    let mut species_counts = Vec::new();
    for (nm, c) in chronica_engine::animals::population_by_species(sim) {
        species_counts.push(serde_json::json!({"n":nm,"c":c}));
    }
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let faiths: Vec<serde_json::Value> = chronica_engine::society::belief_clusters(sim)
        .into_iter()
        .map(|(nm, c)| serde_json::json!({"n":nm,"c":c}))
        .collect();
    let past: Vec<serde_json::Value> = std::fs::read_to_string("/var/lib/chronica/worlds.log")
        .unwrap_or_default()
        .lines()
        .rev()
        .take(50)
        .filter_map(|l| {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 5 {
                Some(serde_json::json!({"no":p[0],"seed":p[1],"year":p[2],"events":p[3],"base":p[4]}))
            } else {
                None
            }
        })
        .collect();

    serde_json::json!({
        "w": g.w, "h": g.h, "day": day, "date": sim.clock.date_string(),
        "cells": cells_b64,
        "people": people, "animals": animals, "buildings": buildings,
        "setts": setts, "marks": marks
    })
    .to_string()
}

/// The live sidebar + DVR buffer range: what the world is *now*, plus how far back you can rewind.
fn render_meta(sim: &Sim, oldest: u64, newest: u64) -> String {
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = Vec::new();
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push(inspection::describe(sim, ev));
            if chron.len() >= 30 {
                break;
            }
        }
    }
    let mut species_counts = Vec::new();
    for (nm, c) in chronica_engine::animals::population_by_species(sim) {
        species_counts.push(serde_json::json!({"n":nm,"c":c}));
    }
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let faiths: Vec<serde_json::Value> = chronica_engine::society::belief_clusters(sim)
        .into_iter()
        .map(|(nm, c)| serde_json::json!({"n":nm,"c":c}))
        .collect();
    let past: Vec<serde_json::Value> = std::fs::read_to_string("/var/lib/chronica/worlds.log")
        .unwrap_or_default()
        .lines()
        .rev()
        .take(50)
        .filter_map(|l| {
            let p: Vec<&str> = l.split('\t').collect();
            if p.len() >= 5 {
                Some(serde_json::json!({"no":p[0],"seed":p[1],"year":p[2],"events":p[3],"base":p[4]}))
            } else {
                None
            }
        })
        .collect();
    serde_json::json!({
        "date": sim.clock.date_string(), "liveDay": sim.clock.day,
        "oldestDay": oldest, "newestDay": newest,
        "stats": {"people": chronica_engine::humans::population(sim),
                   "species": species_counts, "plants": live_plants,
                   "trees": trees, "cover": cover, "events": sim.history.events.len()},
        "faiths": faiths, "past": past, "chron": chron
    })
    .to_string()
}

fn render_png(sim: &Sim) -> Vec<u8> {
    let g = &sim.grid;
    let (w, h) = (g.w as usize, g.h as usize);
    let scale = 5usize;
    let mut px = vec![(0u8, 0u8, 0u8); w * h];
    for i in 0..w * h {
        px[i] = if g.ocean[i] {
            (16, 32, 64)
        } else if g.ice_bears(i) {
            (185, 210, 235)
        } else if g.is_river(i) || g.is_lake(i) {
            (52, 96, 168)
        } else if g.burning[i] > 0 {
            (235, 96, 32)
        } else if g.crop_cover.get(i).copied().unwrap_or(0.0) > 0.15 {
            (212, 178, 70)
        } else if g.is_path(i) {
            (150, 128, 96)
        } else if g.snow[i] > 0.02 {
            (222, 226, 234)
        } else if g.burn_scar[i] > 0.3 {
            (70, 60, 52)
        } else {
            let elev = g.elev[i].clamp(0.0, 1.2);
            let cover = g.veg_cover.get(i).copied().unwrap_or(0.0).min(2.5) / 2.5;
            (
                (96.0 + elev * 90.0 - cover * 60.0).clamp(0.0, 255.0) as u8,
                (92.0 + cover * 110.0 + elev * 20.0).clamp(0.0, 255.0) as u8,
                (58.0 + elev * 40.0 - cover * 30.0).clamp(0.0, 255.0) as u8,
            )
        };
    }
    for b in sim.objects.buildings.iter().filter(|b| b.exists) {
        px[b.cell as usize] = (168, 124, 70);
    }
    for a in sim.animals.list.iter().filter(|a| a.alive) {
        if let Some(i) = g.idx(a.x, a.y) {
            let sp = &chronica_engine::species::ANIMALS[a.species as usize];
            if !sp.aquatic {
                px[i] = if sp.prey.is_empty() { (205, 175, 132) } else { (225, 80, 80) };
            }
        }
    }
    for hu in sim.humans.list.iter().filter(|h| h.alive) {
        if let Some(i) = g.idx(hu.x, hu.y) {
            px[i] = (252, 240, 110);
        }
    }
    // upscale + encode png
    let (ow, oh) = (w * scale, h * scale);
    let mut rgb = Vec::with_capacity(ow * oh * 3);
    for y in 0..oh {
        for x in 0..ow {
            let (r, gg, b) = px[(y / scale) * w + (x / scale)];
            rgb.extend_from_slice(&[r, gg, b]);
        }
    }
    let mut out = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut out, ow as u32, oh as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().unwrap();
        writer.write_image_data(&rgb).unwrap();
    }
    out
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;")
}

fn past_worlds_html() -> String {
    let Ok(t) = std::fs::read_to_string("/var/lib/chronica/worlds.log") else {
        return String::new();
    };
    let mut out = String::new();
    for line in t.lines().rev().take(50) {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() >= 5 {
            out.push_str(&format!(
                "<li>World {} — seed <code>{}</code> — ended year {} — {} events — <a href=\"/archive/{}.png\">portrait</a> · <a href=\"/archive/{}.html\">final page</a></li>",
                parts[0], parts[1], parts[2], parts[3], parts[4], parts[4]
            ));
        }
    }
    if out.is_empty() {
        String::new()
    } else {
        format!("<h2>Worlds that were</h2><ul>{out}</ul>")
    }
}

fn render_html(sim: &Sim) -> String {
    use chronica_engine::core::ids::EntityRef as ER;
    use chronica_engine::history::EventKind as EK;
    let people = chronica_engine::humans::population(sim);
    let children = sim
        .humans
        .list
        .iter()
        .filter(|h| h.alive && (sim.clock.day as i64 - h.born) < 14 * 360)
        .count();
    let mut pops = String::new();
    for (name, c) in chronica_engine::animals::population_by_species(sim) {
        if c > 0 {
            pops.push_str(&format!("<li>{name}: {c}</li>"));
        }
    }
    let extinct: Vec<String> = chronica_engine::animals::population_by_species(sim)
        .into_iter()
        .filter(|(_, c)| *c == 0)
        .map(|(n, _)| n)
        .collect();
    let (live_plants, trees, cover) = chronica_engine::vegetation::forest_stats(sim);
    let mut setts = String::new();
    for (name, (x, y)) in chronica_engine::society::living_settlements(sim) {
        setts.push_str(&format!("<li>{} at ({x},{y})</li>", esc(&name)));
    }
    let mut faiths = String::new();
    let mut forgotten = 0;
    for (name, c) in chronica_engine::society::belief_clusters(sim) {
        if c > 0 {
            faiths.push_str(&format!("<li>{}: {c} faithful</li>", esc(&name)));
        } else {
            forgotten += 1;
        }
    }
    let notable = |ev: &chronica_engine::history::Event| -> bool {
        let human = matches!(ev.subject, ER::Person(_));
        match &ev.kind {
            EK::PlantDied { .. } => false,
            EK::Born { .. } | EK::Died { .. } | EK::Mated { .. } | EK::Ate { .. } => human,
            EK::Killed { by } => human || matches!(by, ER::Person(_)),
            EK::LightningStrike | EK::FireDied => false,
            _ => true,
        }
    };
    let mut chron = String::new();
    let mut shown = 0;
    for ev in sim.history.events.iter().rev() {
        if notable(ev) {
            chron.push_str(&format!("<li>{}</li>", esc(&inspection::describe(sim, ev))));
            shown += 1;
            if shown >= 30 {
                break;
            }
        }
    }
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8">
<meta http-equiv="refresh" content="6">
<title>Chronica — {date}</title>
<style>
body{{background:#14100e;color:#cfc4a6;font-family:'Iowan Old Style',Georgia,serif;margin:0;padding:24px;display:flex;gap:28px;flex-wrap:wrap}}
h1{{font-size:22px;color:#e8dcbc;margin:0 0 4px}} h2{{font-size:15px;color:#b9a2d6;margin:16px 0 4px}}
.map img{{image-rendering:pixelated;border:1px solid #3a2f26;max-width:min(960px,95vw)}}
ul{{margin:4px 0;padding-left:18px;font-size:13px;line-height:1.5}}
.small{{color:#8d8065;font-size:12px}}
.col{{min-width:260px;max-width:420px}}
.chron li{{color:#a8b6a0}}
</style></head><body>
<div class="map">
<h1>CHRONICA — the eternal world</h1>
<div class="small">{date} · this world has been alive since its seed · page refreshes itself</div>
<a href="/map.png"><img src="/map.png?t={day}" alt="the world"></a>
<div class="small">yellow people · tan herds · red predators · gold fields · brown roads &amp; homes · white ice</div>
</div>
<div class="col">
<h2>The living</h2>
<ul><li><b>People: {people}</b> (of whom {children} children)</li>{pops}
<li>Plants: {live_plants} ({trees} grown trees, {coverpct:.0}% forest)</li></ul>
{extinct_html}
<h2>Settlements</h2><ul>{setts_html}</ul>
<h2>Faiths</h2><ul>{faiths}{forgotten_html}</ul>
</div>
<div class="col chron">
<h2>The chronicle (latest notable)</h2><ul>{chron}</ul>
<div class="small">events recorded since the world began: {events}</div>
{past}
</div>
</body></html>"#,
        date = sim.clock.date_string(),
        day = sim.clock.day,
        people = people,
        children = children,
        pops = pops,
        live_plants = live_plants,
        trees = trees,
        coverpct = cover * 100.0,
        extinct_html = if extinct.is_empty() {
            String::new()
        } else {
            format!(
                "<div class=\"small\">gone from the world: {}</div>",
                esc(&extinct.join(", "))
            )
        },
        setts_html = if setts.is_empty() {
            "<li class=\"small\">no settlement yet bears a name</li>".to_string()
        } else {
            setts
        },
        faiths = faiths,
        forgotten_html = if forgotten > 0 {
            format!(
                "<li class=\"small\">…and {forgotten} faiths whose last believer is gone</li>"
            )
        } else {
            String::new()
        },
        chron = chron,
        events = sim.history.events.len(),
        past = past_worlds_html(),
    )
}
