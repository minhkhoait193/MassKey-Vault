import { useEffect, useMemo, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import { createPortal } from 'react-dom';
import { BookOpen, Check, ExternalLink, FolderOpen, Github, Globe2, Pencil, Plus, RotateCw, Search, Trash2, X } from 'lucide-react';

export type Resource = { id:string; url:string; title:string; description:string; notes:string; tags:string[]; collection:string; kind:'website'|'github'|'document'; folderPath:string; docsUrl:string; docsChecked:boolean; stars:number|null; updatedAt:string; metadataStatus:string };
type Metadata = { title:string;description:string;stars:number|null;updatedAt:string;docsUrl:string;status:string };
const empty:Resource={id:'',url:'',title:'',description:'',notes:'',tags:[],collection:'',kind:'website',folderPath:'',docsUrl:'',docsChecked:false,stars:null,updatedAt:'',metadataStatus:''};
const call=<T,>(name:string,args?:Record<string,unknown>)=>invoke<T>(name,args);
const message=(error:unknown)=>error instanceof Error?error.message:String(error);

export default function ResourceSpace({resources,collection,reload,notify,t}:{resources:Resource[];collection:string;reload:()=>Promise<void>;notify:(text:string,failure?:boolean)=>void;t:(text:string)=>string}) {
  const [draft,setDraft]=useState<Resource|null>(null);
  const dialogOpen=!!draft;
  const [closing,setClosing]=useState(false);
  const [query,setQuery]=useState('');
  const [kind,setKind]=useState<'all'|Resource['kind']>('all');
  const [busy,setBusy]=useState(false);
  const [metadataBusy,setMetadataBusy]=useState(false);
  const [metadataHint,setMetadataHint]=useState('');
  const dialogRef=useRef<HTMLElement>(null);
  const returnFocus=useRef<HTMLElement|null>(null);
  const closeTimer=useRef<ReturnType<typeof setTimeout>|null>(null);
  const edit=(resource:Resource)=>{returnFocus.current=document.activeElement as HTMLElement;setClosing(false);setDraft(resource);setMetadataHint('');};
  const close=()=>{
    if(closing)return;
    const finish=()=>{setDraft(null);setClosing(false);};
    if(window.matchMedia('(prefers-reduced-motion: reduce)').matches)finish();
    else {setClosing(true);closeTimer.current=setTimeout(finish,150);}
  };
  useEffect(()=>()=>{if(closeTimer.current)clearTimeout(closeTimer.current);},[]);
  useEffect(()=>{if(!dialogOpen&&returnFocus.current){returnFocus.current.focus();returnFocus.current=null;}},[dialogOpen]);
  useEffect(()=>{
    if(!dialogOpen)return;
    const onKey=(event:KeyboardEvent)=>{
      if(event.key==='Escape'){event.preventDefault();event.stopPropagation();close();}
      if(event.key!=='Tab')return;
      const nodes=Array.from(dialogRef.current?.querySelectorAll<HTMLElement>('button:not(:disabled),input:not(:disabled),select:not(:disabled),textarea:not(:disabled)')??[]);
      if(!nodes.length)return;
      if(event.shiftKey&&document.activeElement===nodes[0]){event.preventDefault();nodes[nodes.length-1].focus();}
      else if(!event.shiftKey&&document.activeElement===nodes[nodes.length-1]){event.preventDefault();nodes[0].focus();}
    };
    document.addEventListener('keydown',onKey);
    return()=>document.removeEventListener('keydown',onKey);
  },[dialogOpen,closing]);
  const filtered=useMemo(()=>resources.filter(resource=>{
    if(collection && resource.collection!==collection)return false;
    if(kind!=='all'&&resource.kind!==kind)return false;
    const text=[resource.title,resource.url,resource.description,resource.notes,...resource.tags].join(' ').toLocaleLowerCase();
    return text.includes(query.trim().toLocaleLowerCase());
  }),[resources,collection,kind,query]);
  useEffect(()=>{if(!draft?.url || draft.title)return;setMetadataHint('');},[draft?.url,draft?.title]);
  const fetchMetadata=async(rawUrl?:string)=>{
    const url=rawUrl||draft?.url;
    if(!url)return;
    setMetadataBusy(true);setMetadataHint('');
    try {
      const data=await call<Metadata>('fetch_resource_metadata',{url});
      setDraft(current=>current?.url===url?{...current,title:current.title||data.title,description:current.description||data.description,
        docsUrl:current.docsUrl||data.docsUrl,docsChecked:true,stars:data.stars,updatedAt:data.updatedAt,metadataStatus:'ok',
        kind:new URL(current.url).hostname.toLowerCase()==='github.com'?'github':current.kind}:current);
      setMetadataHint(t('Đã lấy thông tin. Bạn có thể sửa mọi trường trước khi lưu.'));
    } catch(error) {setMetadataHint(`${t('Không lấy được thông tin')}: ${message(error)}. ${t('Bạn vẫn có thể nhập thủ công.')}`);}
    finally {setMetadataBusy(false);}
  };
  const save=async()=>{
    if(!draft)return;
    setBusy(true);
    try {await call('upsert_resource',{resource:{...draft,title:draft.title.trim(),url:draft.url.trim(),tags:draft.tags.map(tag=>tag.trim()).filter(Boolean)}});
      close();await reload();notify(t('Đã lưu tài nguyên trong vault mã hóa.'));}
    catch(error){notify(message(error),true);}finally{setBusy(false);}
  };
  const act=async(name:string,id:string,success:string)=>{
    setBusy(true);try {await call(name,{id});await reload();notify(success);}catch(error){notify(message(error),true);}finally{setBusy(false);}
  };
  const openItem=async(id:string,docs=false)=>{try{await call('open_resource_url',{id,docs});}catch(error){notify(message(error),true);}};
  const openFolder=async(id:string)=>{try{await call('open_resource_folder',{id});}catch(error){notify(message(error),true);}};
  const refreshRepo=async(resource:Resource)=>{setBusy(true);try{
    const data=await call<Metadata>('fetch_resource_metadata',{url:resource.url,discoverDocs:!resource.docsChecked});
    const current=(await call<{resources:Resource[]}>('snapshot')).resources.find(item=>item.id===resource.id);
    if(current?.url===resource.url)await call('upsert_resource',{resource:{...current,stars:data.stars,updatedAt:data.updatedAt,docsUrl:current.docsUrl||data.docsUrl,docsChecked:true,metadataStatus:'ok'}});
    await reload();notify(t('Đã cập nhật thông tin GitHub.'));
  }catch(error){notify(`${t('Không thể làm mới GitHub')}: ${message(error)}. ${t('Dữ liệu cũ vẫn được giữ.')}`,true);}finally{setBusy(false);}};
  return <div className="workspace resource-space animate-enter">
    <div className="resource-heading"><div><h1>{collection||t('Tài nguyên đã lưu')}<span className="header-dot">.</span></h1><p className="description">{t('Website, tài liệu và repo bạn muốn tìm lại — được lưu trong vault mã hóa.')}</p></div><button className="primary" onClick={()=>edit({...empty,collection})}><Plus size={17}/>{t('Thêm tài nguyên')}</button></div>
    <div className="resource-tools"><label className="search-wrap"><Search size={18}/><input value={query} onChange={event=>setQuery(event.target.value)} placeholder={t('Tìm tiêu đề, URL, ghi chú hoặc thẻ')} aria-label={t('Tìm tài nguyên')}/></label><div className="resource-filters" aria-label={t('Lọc tài nguyên')}>{([['all','Tất cả'],['website','Website'],['github','GitHub'],['document','Tài liệu']] as const).map(([id,label])=><button key={id} className={kind===id?'active':''} aria-pressed={kind===id} onClick={()=>setKind(id)}>{t(label)}</button>)}</div></div>
    {filtered.length===0?<div className="empty-state"><BookOpen size={31}/><h2>{query||kind!=='all'?t('Không tìm thấy tài nguyên'):t('Chưa có tài nguyên')}</h2><p>{query||kind!=='all'?t('Thử từ khóa hoặc bộ lọc khác.'):t('Lưu website, tài liệu hoặc repo đầu tiên để bắt đầu.')}</p></div>:<div className="resource-list">{filtered.map(resource=><article className="resource-card" key={resource.id}>
      <div className="resource-card-icon">{resource.kind==='github'?<Github size={21}/>:resource.kind==='document'?<BookOpen size={21}/>:<Globe2 size={21}/>}</div>
      <div className="resource-card-main"><div className="resource-card-title"><h2>{resource.title}</h2><span>{t(resource.kind==='github'?'GitHub':resource.kind==='document'?'Tài liệu':'Website')}</span></div><p>{resource.description||resource.url}</p><small>{resource.url}</small><div className="resource-card-meta">{resource.collection&&<span>{resource.collection}</span>}{resource.tags.map(tag=><span key={tag}>#{tag}</span>)}{resource.kind==='github'&&<span>★ {resource.stars??'—'}{resource.updatedAt&&` · ${resource.updatedAt.slice(0,10)}`}</span>}{resource.metadataStatus&&resource.metadataStatus!=='ok'&&<span>{t(resource.metadataStatus)}</span>}</div></div>
      <div className="resource-card-actions"><button className="btn-outline" onClick={()=>openItem(resource.id)}><ExternalLink size={15}/>{t('Mở web')}</button>{resource.kind==='github'&&resource.docsChecked&&!resource.docsUrl&&<span className="resource-no-docs" title={t('Có thể thêm URL tài liệu trong phần Sửa')}>{t('Chưa tìm thấy tài liệu')}</span>}{resource.docsUrl&&<button className="btn-outline" onClick={()=>openItem(resource.id,true)}><BookOpen size={15}/>{t('Mở tài liệu')}</button>}{resource.folderPath&&<button className="btn-outline" onClick={()=>openFolder(resource.id)}><FolderOpen size={15}/>{t('Mở thư mục')}</button>}{resource.kind==='github'&&<button className="icon-btn" aria-label={`Làm mới GitHub ${resource.title}`} title={t('Làm mới GitHub')} disabled={busy} onClick={()=>refreshRepo(resource)}><RotateCw size={16}/></button>}<button className="icon-btn" aria-label={`Sửa ${resource.title}`} title={t('Sửa')} onClick={()=>edit({...resource})}><Pencil size={16}/></button><button className="icon-btn" aria-label={`Xóa ${resource.title}`} title={t('Xóa')} disabled={busy} onClick={()=>{if(window.confirm(`Chuyển “${resource.title}” vào Thùng rác?`))act('delete_resource',resource.id,'Đã chuyển tài nguyên vào Thùng rác.');}}><Trash2 size={16}/></button></div>
    </article>)}</div>}
    {draft&&createPortal(<div className={'resource-editor-backdrop'+(closing?' is-closing':'')}><section ref={dialogRef} className="resource-editor" role="dialog" aria-modal="true" aria-label={t(draft.id?'Sửa tài nguyên':'Thêm tài nguyên')}>
      <div className="resource-editor-head"><div><h2>{t(draft.id?'Sửa tài nguyên':'Thêm tài nguyên')}</h2><p>{t('Mọi thông tin được lưu trong vault mã hóa.')}</p></div><button className="icon-btn" aria-label={t('Đóng')} onClick={close}><X size={19}/></button></div>
      <div className="resource-editor-fields"><label>{t('Đường dẫn web')} *<input autoFocus type="url" value={draft.url} onChange={e=>setDraft({...draft,url:e.target.value,docsChecked:false,stars:null,updatedAt:'',metadataStatus:''})} onPaste={event=>{const pasted=event.clipboardData.getData('text').trim();if(/^https:\/\//i.test(pasted)){event.preventDefault();setDraft(current=>current?{...current,url:pasted,docsChecked:false,stars:null,updatedAt:'',metadataStatus:''}:current);void fetchMetadata(pasted);}}} onBlur={()=>{if(draft.url&&!draft.title&&!metadataBusy)void fetchMetadata();}} placeholder="https://example.com"/></label><button className="btn-outline" disabled={metadataBusy||!draft.url} onClick={()=>fetchMetadata()}><RotateCw size={15}/>{metadataBusy?t('Đang lấy thông tin…'):t('Lấy tiêu đề & mô tả')}</button>{metadataHint&&<p className="resource-metadata-hint" role="status">{metadataHint}</p>}
      <label>{t('Tiêu đề')} *<input value={draft.title} onChange={e=>setDraft({...draft,title:e.target.value})}/></label><label>{t('Mô tả ngắn')}<input value={draft.description} onChange={e=>setDraft({...draft,description:e.target.value})}/></label><label>{t('Loại')}<select value={draft.kind} onChange={e=>setDraft({...draft,kind:e.target.value as Resource['kind']})}><option value="website">{t('Website')}</option><option value="github">GitHub</option><option value="document">{t('Tài liệu')}</option></select></label><label>{t('Bộ sưu tập')}<input value={draft.collection} onChange={e=>setDraft({...draft,collection:e.target.value})} placeholder={t('Ví dụ: Học tập')}/></label><label>{t('Thẻ (ngăn bằng dấu phẩy)')}<input value={draft.tags.join(', ')} onChange={e=>setDraft({...draft,tags:e.target.value.split(',')})}/></label><label>{t('URL tài liệu')}<input type="url" value={draft.docsUrl} onChange={e=>setDraft({...draft,docsUrl:e.target.value,docsChecked:!!e.target.value})} placeholder="https://..."/></label><label>{t('Thư mục Windows (tùy chọn)')}<div className="resource-folder-field"><input value={draft.folderPath} onChange={e=>setDraft({...draft,folderPath:e.target.value})} placeholder="D:\\Tai-lieu"/><button className="btn-outline" onClick={async()=>{const path=await open({directory:true,multiple:false});if(typeof path==='string')setDraft(current=>current?{...current,folderPath:path}:current);}}><FolderOpen size={16}/>{t('Chọn')}</button></div></label><p className="resource-field-note">{t('Chỉ lưu đường dẫn. Sau khi khôi phục trên máy khác, hãy kiểm tra lại thư mục.')}</p><label>{t('Ghi chú cá nhân')}<textarea rows={4} value={draft.notes} onChange={e=>setDraft({...draft,notes:e.target.value})}/></label></div>
      <div className="resource-editor-actions"><button className="btn-outline" onClick={close}>{t('Hủy')}</button><button className="primary" disabled={busy||!draft.title.trim()||!draft.url.trim()} onClick={save}><Check size={16}/>{t('Lưu tài nguyên')}</button></div>
    </section></div>,document.body)}
  </div>;
}

export function ResourceTrash({items,reload,notify,t}:{items:Resource[];reload:()=>Promise<void>;notify:(text:string,failure?:boolean)=>void;t:(text:string)=>string}) {
  const [busy,setBusy]=useState(false);
  const action=async(name:string,id:string)=>{setBusy(true);try{await call(name,{id});await reload();notify(t(name==='restore_resource'?'Đã khôi phục tài nguyên.':'Đã xóa khỏi vault hiện tại.'));}catch(error){notify(message(error),true);}finally{setBusy(false);}};
  return <section className="resource-trash-section"><h2>{t('Tài nguyên đã xóa')} ({items.length})</h2>{items.length===0?<p>{t('Không có tài nguyên trong thùng rác.')}</p>:items.map(item=><div className="resource-trash-row" key={item.id}><span>{item.title}</span><button className="btn-outline" disabled={busy} onClick={()=>action('restore_resource',item.id)}>{t('Khôi phục')}</button><button className="btn-outline danger-action" disabled={busy} onClick={()=>{if(window.confirm(`Xóa vĩnh viễn “${item.title}” khỏi vault hiện tại? Bản sao cũ vẫn có thể chứa mục này.`))action('permanently_delete_resource',item.id);}}>{t('Xóa vĩnh viễn')}</button></div>)}</section>;
}
