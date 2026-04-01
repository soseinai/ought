use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

use axum::{
    Router,
    response::{Html, Json},
    routing::get,
};
use serde_json::{Value, json};

use ought_spec::{Clause, Config, Keyword, Section, Spec, SpecGraph};

// ─── JSON serialization ────────────────────────────────────────────────────

fn spec_to_json(spec: &Spec) -> Value {
    json!({
        "name": spec.name,
        "source_path": spec.source_path.display().to_string(),
        "metadata": {
            "context": spec.metadata.context,
            "sources": spec.metadata.sources,
            "schemas": spec.metadata.schemas,
            "requires": spec.metadata.requires.iter().map(|r| json!({
                "label": r.label,
                "path": r.path.display().to_string(),
                "anchor": r.anchor,
            })).collect::<Vec<_>>(),
        },
        "sections": spec.sections.iter().map(section_to_json).collect::<Vec<_>>(),
    })
}

fn section_to_json(section: &Section) -> Value {
    json!({
        "title": section.title,
        "depth": section.depth,
        "prose": section.prose,
        "clauses": section.clauses.iter().map(clause_to_json).collect::<Vec<_>>(),
        "subsections": section.subsections.iter().map(section_to_json).collect::<Vec<_>>(),
    })
}

fn clause_to_json(clause: &Clause) -> Value {
    let temporal = clause.temporal.as_ref().map(|t| match t {
        ought_spec::Temporal::Invariant => json!({ "kind": "invariant" }),
        ought_spec::Temporal::Deadline(dur) => json!({ "kind": "deadline", "duration": format!("{:?}", dur) }),
    });

    json!({
        "id": clause.id.0,
        "keyword": format!("{:?}", clause.keyword),
        "severity": format!("{:?}", clause.severity),
        "text": clause.text,
        "condition": clause.condition,
        "otherwise": clause.otherwise.iter().map(clause_to_json).collect::<Vec<_>>(),
        "temporal": temporal,
        "hints": clause.hints,
    })
}

fn keyword_display(kw: &Keyword) -> &'static str {
    match kw {
        Keyword::Must => "Must",
        Keyword::MustNot => "MustNot",
        Keyword::Should => "Should",
        Keyword::ShouldNot => "ShouldNot",
        Keyword::May => "May",
        Keyword::Wont => "Wont",
        Keyword::Given => "Given",
        Keyword::Otherwise => "Otherwise",
        Keyword::MustAlways => "MustAlways",
        Keyword::MustBy => "MustBy",
    }
}

fn count_clauses(sections: &[Section]) -> usize {
    sections
        .iter()
        .map(|s| {
            s.clauses.len()
                + s.clauses.iter().map(|c| c.otherwise.len()).sum::<usize>()
                + count_clauses(&s.subsections)
        })
        .sum()
}

fn count_sections(sections: &[Section]) -> usize {
    sections
        .iter()
        .map(|s| 1 + count_sections(&s.subsections))
        .sum()
}

fn count_by_keyword(sections: &[Section], counts: &mut HashMap<&'static str, usize>) {
    for section in sections {
        for clause in &section.clauses {
            *counts.entry(keyword_display(&clause.keyword)).or_insert(0) += 1;
            for ow in &clause.otherwise {
                *counts.entry(keyword_display(&ow.keyword)).or_insert(0) += 1;
            }
        }
        count_by_keyword(&section.subsections, counts);
    }
}

fn build_api_response(specs: &[Spec]) -> Value {
    let total_specs = specs.len();
    let total_sections: usize = specs.iter().map(|s| count_sections(&s.sections)).sum();
    let total_clauses: usize = specs.iter().map(|s| count_clauses(&s.sections)).sum();

    let mut by_keyword: HashMap<&str, usize> = HashMap::new();
    for spec in specs {
        count_by_keyword(&spec.sections, &mut by_keyword);
    }

    json!({
        "specs": specs.iter().map(spec_to_json).collect::<Vec<_>>(),
        "stats": {
            "total_specs": total_specs,
            "total_sections": total_sections,
            "total_clauses": total_clauses,
            "by_keyword": by_keyword,
        },
    })
}

// ─── HTML template ─────────────────────────────────────────────────────────

const VIEWER_HTML: &str = r##"<!DOCTYPE html>
<html lang="en" data-theme="dark">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>ought viewer</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
<link href="https://fonts.googleapis.com/css2?family=Exo+2:wght@500;600;700;800&family=Instrument+Sans:wght@400;500;600&family=Charis+SIL:wght@400;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
<style>
*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}

/* Warm monochrome tokens — 1960s space / 2001 */
[data-theme="light"]{
  --background:40 6% 98%;--foreground:30 8% 8%;
  --card:40 6% 100%;--card-foreground:30 8% 8%;
  --muted:35 5% 95%;--muted-foreground:30 4% 46%;
  --border:35 6% 89%;--input:35 6% 89%;
  --ring:30 6% 20%;--radius:0.5rem;
  --accent:35 5% 95%;--accent-foreground:30 6% 10%;
  --popover:40 6% 100%;
  --kw-must-bg:0 72% 94%;--kw-must-fg:0 84% 44%;
  --kw-should-bg:43 96% 93%;--kw-should-fg:43 96% 38%;
  --kw-may-bg:240 5% 95%;--kw-may-fg:240 4% 46%;
  --kw-wont-bg:270 76% 95%;--kw-wont-fg:270 76% 48%;
  --kw-given-bg:197 60% 93%;--kw-given-fg:197 60% 33%;
  --kw-otherwise-bg:25 90% 94%;--kw-otherwise-fg:25 90% 45%;
  --kw-temporal-bg:152 60% 93%;--kw-temporal-fg:152 60% 32%;
}

[data-theme="dark"]{
  --background:30 6% 5%;--foreground:35 5% 93%;
  --card:30 5% 7%;--card-foreground:35 5% 93%;
  --muted:30 4% 14%;--muted-foreground:30 4% 58%;
  --border:30 4% 14%;--input:30 4% 14%;
  --ring:35 5% 80%;--radius:0.5rem;
  --accent:30 4% 14%;--accent-foreground:35 5% 93%;
  --popover:30 6% 5%;
  --kw-must-bg:0 72% 12%;--kw-must-fg:0 84% 68%;
  --kw-should-bg:43 80% 11%;--kw-should-fg:43 96% 65%;
  --kw-may-bg:240 4% 16%;--kw-may-fg:240 5% 65%;
  --kw-wont-bg:270 60% 14%;--kw-wont-fg:270 76% 72%;
  --kw-given-bg:197 50% 13%;--kw-given-fg:197 60% 62%;
  --kw-otherwise-bg:25 70% 12%;--kw-otherwise-fg:25 90% 66%;
  --kw-temporal-bg:152 50% 12%;--kw-temporal-fg:152 60% 58%;
}

body{font-family:"Instrument Sans",ui-sans-serif,system-ui,-apple-system,sans-serif;
background:hsl(var(--background));color:hsl(var(--foreground));
line-height:1.6;display:flex;flex-direction:column;height:100vh;
-webkit-font-smoothing:antialiased;-moz-osx-font-smoothing:grayscale;font-size:14px;
letter-spacing:-.01em}
a{color:hsl(var(--foreground));text-decoration:underline;text-underline-offset:4px}
a:hover{opacity:.8}
::selection{background:hsl(var(--foreground));color:hsl(var(--background))}

/* Header */
.header{background:hsl(var(--card));border-bottom:1px solid hsl(var(--border));
padding:0 20px;display:flex;align-items:center;gap:16px;flex-shrink:0;height:52px}
.header h1{font-family:"Exo 2",sans-serif;font-size:15px;font-weight:700;letter-spacing:.08em;
text-transform:uppercase;display:flex;align-items:center;gap:6px}
.header h1 .logo{opacity:.4;font-weight:500;letter-spacing:.1em}
.header .stats{font-size:12px;color:hsl(var(--muted-foreground));margin-left:auto;display:flex;gap:12px;align-items:center}
.header .stats span{white-space:nowrap}
.icon-btn{background:transparent;border:1px solid hsl(var(--border));color:hsl(var(--foreground));
cursor:pointer;padding:6px;border-radius:calc(var(--radius) - 2px);display:inline-flex;
align-items:center;justify-content:center;transition:background .15s,border-color .15s}
.icon-btn:hover{background:hsl(var(--accent));border-color:hsl(var(--accent))}
.icon-btn svg{width:15px;height:15px;stroke:currentColor;stroke-width:2;stroke-linecap:round;
stroke-linejoin:round;fill:none}
[data-theme="dark"] .icon-sun{display:none}[data-theme="light"] .icon-moon{display:none}

/* Search */
.search-bar{padding:10px 20px;background:hsl(var(--card));border-bottom:1px solid hsl(var(--border));
flex-shrink:0;display:flex;gap:8px;align-items:center;flex-wrap:wrap}
.search-bar input{flex:1;min-width:200px;padding:8px 12px;border:1px solid hsl(var(--input));
border-radius:calc(var(--radius) - 2px);font-size:13px;outline:none;
background:hsl(var(--background));color:hsl(var(--foreground));transition:box-shadow .15s}
.search-bar input:focus{box-shadow:0 0 0 2px hsl(var(--ring) / .2);border-color:hsl(var(--ring) / .4)}
.search-bar input::placeholder{color:hsl(var(--muted-foreground))}
.filter-pills{display:flex;gap:4px;flex-wrap:wrap}
.filter-pill{padding:1px 8px;border-radius:calc(var(--radius) - 2px);font-size:10px;font-weight:600;
cursor:pointer;border:1px solid transparent;opacity:.45;transition:all .15s;letter-spacing:.3px}
.filter-pill:hover{opacity:.75}.filter-pill.active{opacity:1;border-color:currentColor}

/* Layout */
.layout{display:flex;flex:1;overflow:hidden}
.sidebar{width:260px;min-width:220px;background:hsl(var(--card));border-right:1px solid hsl(var(--border));
overflow-y:auto;flex-shrink:0;padding:8px 0}
.sidebar::-webkit-scrollbar{width:4px}.sidebar::-webkit-scrollbar-thumb{background:hsl(var(--border));border-radius:2px}
.main{flex:1;overflow-y:auto;padding:32px 40px}
.main::-webkit-scrollbar{width:6px}.main::-webkit-scrollbar-thumb{background:hsl(var(--border));border-radius:3px}

/* Sidebar tree */
.tree-item{padding:6px 12px;cursor:pointer;font-size:13px;display:flex;align-items:center;gap:6px;
color:hsl(var(--foreground));transition:all .1s;white-space:nowrap;overflow:hidden;text-overflow:ellipsis;
border-radius:calc(var(--radius) - 2px);margin:1px 8px}
.tree-item:hover{background:hsl(var(--accent))}
.tree-item.active{background:hsl(var(--accent));font-weight:500}
.tree-item .arrow{width:14px;height:14px;transition:transform .15s;flex-shrink:0;
color:hsl(var(--muted-foreground));display:inline-flex;align-items:center;justify-content:center}
.tree-item .arrow svg{width:12px;height:12px;stroke:currentColor;stroke-width:2;stroke-linecap:round;
stroke-linejoin:round;fill:none}
.tree-item .arrow.open{transform:rotate(90deg)}
.tree-section{padding-left:24px;font-size:12px;color:hsl(var(--muted-foreground));margin:1px 8px}
.tree-children{display:none}.tree-children.open{display:block}
.tree-count{font-size:11px;color:hsl(var(--muted-foreground));margin-left:auto;padding-right:2px;flex-shrink:0;
font-variant-numeric:tabular-nums}

/* Spec header */
.spec-header h2{font-family:"Exo 2",sans-serif;font-size:22px;font-weight:700;margin-bottom:2px;letter-spacing:.02em}
.spec-path{font-size:12px;color:hsl(var(--muted-foreground));margin-bottom:20px;
font-family:"JetBrains Mono",ui-monospace,SFMono-Regular,Menlo,monospace;font-size:11px}
.meta-block{background:hsl(var(--muted));border:1px solid hsl(var(--border));
border-radius:var(--radius);padding:12px 16px;margin-bottom:24px;font-size:13px;
color:hsl(var(--muted-foreground))}
.meta-block .meta-row{margin-bottom:3px}.meta-block .meta-label{font-weight:500;color:hsl(var(--foreground))}

/* Section card */
.section-card{background:hsl(var(--card));border:1px solid hsl(var(--border));
border-radius:var(--radius);margin-bottom:12px;overflow:hidden}
.section-head{padding:12px 16px;cursor:pointer;display:flex;align-items:center;gap:8px;
font-family:"Exo 2",sans-serif;font-weight:600;font-size:14px;user-select:none;
color:hsl(var(--foreground));letter-spacing:.02em}
.section-head:hover{background:hsl(var(--accent))}
.section-head .arrow{transition:transform .15s;color:hsl(var(--muted-foreground));display:inline-flex}
.section-head .arrow svg{width:14px;height:14px;stroke:currentColor;stroke-width:2;stroke-linecap:round;
stroke-linejoin:round;fill:none}
.section-head .arrow.open{transform:rotate(90deg)}
.section-head .clause-count{font-size:11px;color:hsl(var(--muted-foreground));font-weight:400;margin-left:auto;
font-variant-numeric:tabular-nums}
.section-body{padding:4px 16px 12px;display:none}
.section-body.open{display:block}
.section-prose{font-size:13px;color:hsl(var(--muted-foreground));margin-bottom:10px;padding:8px 12px;
background:hsl(var(--muted));border-radius:calc(var(--radius) - 2px);border-left:2px solid hsl(var(--border))}
.subsection{margin-left:8px;border-left:1px solid hsl(var(--border));padding-left:16px;margin-top:14px}
.subsection-title{font-family:"Exo 2",sans-serif;font-weight:600;font-size:13px;margin-bottom:6px;
color:hsl(var(--foreground));letter-spacing:.02em}

/* Clause */
.clause{display:flex;gap:10px;padding:8px 2px;border-bottom:1px solid hsl(var(--border) / .5);align-items:baseline}
.clause:last-child{border-bottom:none}
.clause-condition{font-size:11px;color:hsl(var(--kw-given-fg));padding:6px 0 2px;font-weight:500;
font-family:ui-monospace,SFMono-Regular,monospace;letter-spacing:-.2px}
.clause-text{flex:1;font-family:"Charis SIL",Charter,Georgia,serif;font-size:14px;line-height:1.6;
color:hsl(var(--foreground) / .9)}
.clause-hints{margin-top:6px}
.clause-hint{background:hsl(var(--muted));border:1px solid hsl(var(--border));
border-radius:calc(var(--radius) - 2px);padding:8px 12px;
font-family:ui-monospace,SFMono-Regular,"SF Mono",Menlo,monospace;font-size:12px;margin-top:4px;
white-space:pre-wrap;word-break:break-all;line-height:1.5;color:hsl(var(--foreground) / .8)}

/* Otherwise chain */
.otherwise-chain{margin-left:28px;border-left:1px dashed hsl(var(--border));padding-left:12px;margin-top:4px}
.otherwise-chain .clause{opacity:.8}

/* Keyword badges — ShadCN outline badge style */
.kw{display:inline-block;padding:2px 8px;border-radius:calc(var(--radius) - 2px);font-size:10px;
font-weight:600;white-space:nowrap;flex-shrink:0;text-transform:uppercase;letter-spacing:.06em;
min-width:48px;text-align:center;border:1px solid transparent}
.kw-Must,.kw-MustNot,.kw-MustAlways,.kw-MustBy{
  background:hsl(var(--kw-must-bg));color:hsl(var(--kw-must-fg));border-color:hsl(var(--kw-must-fg) / .2)}
.kw-Should,.kw-ShouldNot{
  background:hsl(var(--kw-should-bg));color:hsl(var(--kw-should-fg));border-color:hsl(var(--kw-should-fg) / .2)}
.kw-May{
  background:hsl(var(--kw-may-bg));color:hsl(var(--kw-may-fg));border-color:hsl(var(--kw-may-fg) / .2)}
.kw-Wont{
  background:hsl(var(--kw-wont-bg));color:hsl(var(--kw-wont-fg));border-color:hsl(var(--kw-wont-fg) / .2)}
.kw-Given{
  background:hsl(var(--kw-given-bg));color:hsl(var(--kw-given-fg));border-color:hsl(var(--kw-given-fg) / .2)}
.kw-Otherwise{
  background:hsl(var(--kw-otherwise-bg));color:hsl(var(--kw-otherwise-fg));border-color:hsl(var(--kw-otherwise-fg) / .2)}

/* Temporal badges */
.temporal{font-size:10px;padding:1px 6px;border-radius:calc(var(--radius) - 2px);
background:hsl(var(--kw-temporal-bg));color:hsl(var(--kw-temporal-fg));
border:1px solid hsl(var(--kw-temporal-fg) / .2);margin-left:6px;font-weight:600;letter-spacing:.2px}

/* Responsive */
@media(max-width:768px){.sidebar{display:none}.main{padding:16px 20px}}

/* Transitions */
.section-card,.tree-item,.clause,.theme-toggle{transition:background .1s,border-color .15s}
</style>
</head>
<body>
<div class="header">
  <h1>ought <span class="logo">/ viewer</span></h1>
  <div class="stats" id="stats"></div>
  <button class="icon-btn" onclick="toggleTheme()" id="theme-btn" title="Toggle theme"><svg class="icon-sun" viewBox="0 0 24 24"><circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M6.34 17.66l-1.41 1.41M19.07 4.93l-1.41 1.41"/></svg><svg class="icon-moon" viewBox="0 0 24 24"><path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z"/></svg></button>
</div>
<div class="search-bar">
  <svg style="width:15px;height:15px;stroke:hsl(var(--muted-foreground));stroke-width:2;stroke-linecap:round;stroke-linejoin:round;fill:none;flex-shrink:0" viewBox="0 0 24 24"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.3-4.3"/></svg>
  <input type="text" id="search" placeholder="Search clauses...">
  <div class="filter-pills" id="filters"></div>
</div>
<div class="layout">
  <div class="sidebar" id="sidebar"></div>
  <div class="main" id="main"><p style="color:#999;padding:40px">Loading specs...</p></div>
</div>
<script>
let DATA=null,activeSpec=null,activeFilter=null;

function toggleTheme(){
  const html=document.documentElement;
  const current=html.getAttribute("data-theme");
  const next=current==="dark"?"light":"dark";
  html.setAttribute("data-theme",next);
  localStorage.setItem("ought-theme",next);
}
// Restore saved theme (dark is default)
(function(){
  const saved=localStorage.getItem("ought-theme");
  if(saved){document.documentElement.setAttribute("data-theme",saved)}
})();
const KW_LABELS={Must:"MUST",MustNot:"MUST NOT",Should:"SHOULD",ShouldNot:"SHOULD NOT",
  May:"MAY",Wont:"WONT",Given:"GIVEN",Otherwise:"OTHERWISE",MustAlways:"MUST ALWAYS",MustBy:"MUST BY"};

async function init(){
  const r=await fetch("/api/specs");DATA=await r.json();
  renderStats();renderFilters();renderSidebar();
  if(DATA.specs.length)selectSpec(0);
}

function renderStats(){
  const s=DATA.stats,el=document.getElementById("stats");
  el.innerHTML=`<span>${s.total_specs} specs</span><span>${s.total_sections} sections</span><span>${s.total_clauses} clauses</span>`;
  const bk=s.by_keyword;
  for(const[k,v]of Object.entries(bk)){el.innerHTML+=`<span class="kw kw-${k}" style="font-size:11px">${KW_LABELS[k]||k} ${v}</span>`}
}

function renderFilters(){
  const el=document.getElementById("filters");
  const kws=["Must","MustNot","Should","ShouldNot","May","Wont","Given","Otherwise","MustAlways","MustBy"];
  kws.forEach(k=>{
    if(!DATA.stats.by_keyword[k])return;
    const pill=document.createElement("span");
    pill.className=`filter-pill kw kw-${k}`;pill.textContent=KW_LABELS[k]||k;
    pill.onclick=()=>{
      if(activeFilter===k){activeFilter=null;pill.classList.remove("active")}
      else{document.querySelectorAll(".filter-pill").forEach(p=>p.classList.remove("active"));activeFilter=k;pill.classList.add("active")}
      renderMain();
    };el.appendChild(pill);
  });
}

function renderSidebar(){
  const el=document.getElementById("sidebar");el.innerHTML="";
  DATA.specs.forEach((spec,si)=>{
    const item=document.createElement("div");
    const cc=countSpecClauses(spec);
    item.className="tree-item";item.dataset.idx=si;
    item.innerHTML=`<span class="arrow"><svg viewBox="0 0 24 24"><path d="m9 18 6-6-6-6"/></svg></span><svg style="width:14px;height:14px;stroke:currentColor;stroke-width:2;stroke-linecap:round;stroke-linejoin:round;fill:none;flex-shrink:0;opacity:.5" viewBox="0 0 24 24"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/></svg> ${esc(spec.name)}<span class="tree-count">${cc}</span>`;
    item.onclick=e=>{e.stopPropagation();selectSpec(si);toggleTree(item)};
    el.appendChild(item);
    const children=document.createElement("div");children.className="tree-children";
    renderTreeSections(spec.sections,children,si);
    el.appendChild(children);
  });
}

function renderTreeSections(sections,parent,si){
  sections.forEach(sec=>{
    const item=document.createElement("div");item.className="tree-item tree-section";
    item.textContent=sec.title;item.onclick=e=>{e.stopPropagation();selectSpec(si);
      setTimeout(()=>{const t=document.getElementById("sec-"+sec.title.replace(/\s+/g,"-"));if(t)t.scrollIntoView({behavior:"smooth"})},50)};
    parent.appendChild(item);
    if(sec.subsections.length){const ch=document.createElement("div");ch.className="tree-children open";
      renderTreeSections(sec.subsections,ch,si);parent.appendChild(ch)}
  });
}

function toggleTree(item){
  const next=item.nextElementSibling;
  if(next&&next.classList.contains("tree-children")){
    next.classList.toggle("open");item.querySelector(".arrow").classList.toggle("open")}
}

function selectSpec(idx){
  activeSpec=idx;
  document.querySelectorAll(".tree-item[data-idx]").forEach(el=>{
    el.classList.toggle("active",parseInt(el.dataset.idx)===idx);
    if(parseInt(el.dataset.idx)===idx){const a=el.querySelector(".arrow");if(a)a.classList.add("open");
      const n=el.nextElementSibling;if(n&&n.classList.contains("tree-children"))n.classList.add("open")}
  });
  renderMain();
}

function renderMain(){
  const el=document.getElementById("main");
  if(activeSpec===null){el.innerHTML="<p>Select a spec</p>";return}
  const spec=DATA.specs[activeSpec],q=document.getElementById("search").value.toLowerCase();
  let h=`<div class="spec-header"><h2>${esc(spec.name)}</h2></div>`;
  h+=`<div class="spec-path">${esc(spec.source_path)}</div>`;
  // metadata
  const m=spec.metadata;
  if(m.context||m.sources.length||m.schemas.length||m.requires.length){
    h+=`<div class="meta-block">`;
    if(m.context)h+=`<div class="meta-row"><span class="meta-label">Context:</span> ${esc(m.context)}</div>`;
    if(m.sources.length)h+=`<div class="meta-row"><span class="meta-label">Sources:</span> ${m.sources.map(esc).join(", ")}</div>`;
    if(m.schemas.length)h+=`<div class="meta-row"><span class="meta-label">Schemas:</span> ${m.schemas.map(esc).join(", ")}</div>`;
    if(m.requires.length)h+=`<div class="meta-row"><span class="meta-label">Requires:</span> ${m.requires.map(r=>esc(r.label||r.path)).join(", ")}</div>`;
    h+=`</div>`;
  }
  h+=renderSections(spec.sections,q);
  el.innerHTML=h;
  // wire section toggles
  el.querySelectorAll(".section-head").forEach(sh=>{
    sh.onclick=()=>{sh.querySelector(".arrow").classList.toggle("open");
      sh.nextElementSibling.classList.toggle("open")}
  });
}

function renderSections(sections,q,depth){
  depth=depth||0;let h="";
  sections.forEach(sec=>{
    const clauses=filterClauses(sec.clauses,q);
    const subHtml=renderSections(sec.subsections,q,depth+1);
    if(!clauses.length&&!subHtml&&q)return;
    const id="sec-"+sec.title.replace(/\s+/g,"-");
    if(depth>0){
      h+=`<div class="subsection" id="${id}"><div class="subsection-title">${esc(sec.title)}</div>`;
      if(sec.prose)h+=`<div class="section-prose">${esc(sec.prose)}</div>`;
      h+=renderClauseList(clauses)+subHtml+`</div>`;
    }else{
      const cc=sec.clauses.length;
      h+=`<div class="section-card" id="${id}"><div class="section-head"><span class="arrow open"><svg viewBox="0 0 24 24"><path d="m9 18 6-6-6-6"/></svg></span>${esc(sec.title)}<span class="clause-count">${cc} clause${cc!==1?"s":""}</span></div>`;
      h+=`<div class="section-body open">`;
      if(sec.prose)h+=`<div class="section-prose">${esc(sec.prose)}</div>`;
      h+=renderClauseList(clauses)+subHtml+`</div></div>`;
    }
  });
  return h;
}

function filterClauses(clauses,q){
  return clauses.filter(c=>{
    if(activeFilter&&c.keyword!==activeFilter)return false;
    if(q&&!c.text.toLowerCase().includes(q)&&!(c.condition||"").toLowerCase().includes(q)&&!c.id.toLowerCase().includes(q))return false;
    return true;
  });
}

function renderClauseList(clauses){
  if(!clauses.length)return"";
  let h="";
  clauses.forEach(c=>{
    if(c.condition)h+=`<div class="clause-condition">GIVEN: ${esc(c.condition)}</div>`;
    h+=`<div class="clause"><span class="kw kw-${c.keyword}">${KW_LABELS[c.keyword]||c.keyword}</span>`;
    h+=`<div class="clause-text">${esc(c.text)}`;
    if(c.temporal){
      if(c.temporal.kind==="invariant")h+=` <span class="temporal">INVARIANT</span>`;
      else if(c.temporal.kind==="deadline")h+=` <span class="temporal">${esc(c.temporal.duration)}</span>`;
    }
    if(c.hints&&c.hints.length){h+=`<div class="clause-hints">`;c.hints.forEach(hint=>{h+=`<div class="clause-hint">${esc(hint)}</div>`});h+=`</div>`}
    h+=`</div></div>`;
    if(c.otherwise&&c.otherwise.length){h+=`<div class="otherwise-chain">`;c.otherwise.forEach(ow=>{
      h+=`<div class="clause"><span class="kw kw-${ow.keyword}">${KW_LABELS[ow.keyword]||ow.keyword}</span>`;
      h+=`<div class="clause-text">${esc(ow.text)}</div></div>`;
    });h+=`</div>`}
  });
  return h;
}

function countSpecClauses(spec){let n=0;function cs(secs){secs.forEach(s=>{n+=s.clauses.length;s.clauses.forEach(c=>n+=c.otherwise.length);cs(s.subsections)})}cs(spec.sections);return n}
function esc(s){if(!s)return"";return String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;")}

document.getElementById("search").addEventListener("input",()=>renderMain());
init();
</script>
</body>
</html>
"##;

// ─── Server ────────────────────────────────────────────────────────────────

pub fn cmd_view(
    config_path: &Option<PathBuf>,
    port: u16,
    no_open: bool,
) -> anyhow::Result<()> {
    let (cfg_path, config) = match config_path {
        Some(path) => {
            let config = Config::load(path)?;
            (path.clone(), config)
        }
        None => Config::discover()?,
    };

    let config_dir = cfg_path
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();

    let roots: Vec<PathBuf> = config
        .specs
        .roots
        .iter()
        .map(|r| config_dir.join(r))
        .collect();

    let graph = SpecGraph::from_roots(&roots).map_err(|errors| {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        anyhow::anyhow!("spec parse errors:\n  {}", messages.join("\n  "))
    })?;

    let api_json = build_api_response(graph.specs());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async move {
        let json_data = api_json.clone();
        let app = Router::new()
            .route("/", get(|| async { Html(VIEWER_HTML) }))
            .route(
                "/api/specs",
                get(move || {
                    let data = json_data.clone();
                    async move { Json(data) }
                }),
            );

        let addr = SocketAddr::from(([127, 0, 0, 1], port));
        let listener = tokio::net::TcpListener::bind(addr).await?;
        eprintln!("Serving ought viewer at http://localhost:{}", port);

        if !no_open {
            let url = format!("http://localhost:{}", port);
            let _ = std::process::Command::new("open").arg(&url).spawn();
        }

        axum::serve(listener, app).await?;

        Ok::<(), anyhow::Error>(())
    })?;

    Ok(())
}
