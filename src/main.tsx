import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { open, save } from '@tauri-apps/plugin-dialog';
import { getCurrentWebview } from '@tauri-apps/api/webview';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { translate, type Language } from './i18n';
import {
  ArchiveRestore, ArrowDownToLine, ArrowRight, Check, ChevronDown,
  ClipboardCopy, Clock3, Eye, EyeOff, FolderClosed, FolderPlus, KeyRound,
  LayoutGrid, Lock, LockKeyhole, Moon, Pencil, Plus, Search,
  Settings2, Shield, ShieldCheck, ShieldOff, SlidersHorizontal,
  Sun, Trash2, UploadCloud, X, Zap, Languages, ExternalLink, ImagePlus, Activity, RefreshCw
} from 'lucide-react';
import logo from './logo.png';
import './style.css';

type Category = { id: string; name: string; icon: string; color: string };
type Account = { id: string; categoryId: string; title: string; username: string; website: string; notes: string; icon: string };
type Settings = { auto_lock_seconds: number; clipboard_seconds: number; lock_on_minimize:boolean; require_reauth:boolean; warn_weak_passwords:boolean; vault_health:boolean };
type HealthReport = { weak:string[]; reused:string[]; total:number };
type Snapshot = { categories: Category[]; accounts: Account[]; settings: Settings };
type Status = { exists: boolean; unlocked: boolean };
type Tab = 'all' | 'category' | 'settings' | 'backup';
type AccountDraft = { id?: string; title: string; categoryId: string; username: string; password: string; website: string; notes: string; icon: string };
const icons = ['✦','◈','✉','♧','☁','⌘','◎','◉','▣','◇','♛','☼','✳','♥','✿','⚙','▦','♟','♬','✈','🔐','🎮','🏦','💼','📨','🌐'];
const colors = ['#9d82ff','#50cbb7','#e9b36f','#e78aab','#76a9fb','#b9a7ef'];
const blank: AccountDraft = {title:'',categoryId:'',username:'',password:'',website:'',notes:'',icon:'✦'};
const defaultSnap: Snapshot = { categories:[], accounts:[], settings:{auto_lock_seconds:180,clipboard_seconds:25,lock_on_minimize:false,require_reauth:false,warn_weak_passwords:true,vault_health:true} };
const api = <T,>(name: string, args?: Record<string,unknown>) => invoke<T>(name,args);
const errorText = (e: unknown) => typeof e === 'string' ? e : (e instanceof Error ? e.message : 'Có lỗi xảy ra.');


// Custom, keyboard-accessible dropdown avoids the inconsistent native Windows select popup.
const languageOptions: {id:Language;code:string;flag:string;name:string}[] = [
  {id:'vi',code:'VI',flag:'🇻🇳',name:'Tiếng Việt'},
  {id:'th',code:'TH',flag:'🇹🇭',name:'ภาษาไทย'},
  {id:'en',code:'EN',flag:'🇬🇧',name:'English'},
];
function LanguageMenu({value,onChange}:{value:Language;onChange:(value:Language)=>void}) {
  const [open,setOpen] = useState(false);
  const containerRef = useRef<HTMLDivElement>(null);
  const triggerRef = useRef<HTMLButtonElement>(null);
  const selected = languageOptions.find(o=>o.id===value)!;
  useEffect(()=>{
    if(!open)return;
    const onPointer=(e:PointerEvent)=>{
      if(!containerRef.current?.contains(e.target as Node))setOpen(false);
    };
    const onKey=(e:KeyboardEvent)=>{
      if(e.key==='Escape') {e.stopPropagation();setOpen(false);triggerRef.current?.focus();}
    };
    document.addEventListener('pointerdown',onPointer);
    document.addEventListener('keydown',onKey,true);
    return()=>{document.removeEventListener('pointerdown',onPointer);document.removeEventListener('keydown',onKey,true);};
  },[open]);
  useEffect(()=>{
    if(open)containerRef.current?.querySelector<HTMLButtonElement>('[role="menuitemradio"][aria-checked="true"]')?.focus();
  },[open]);
  const moveFocus=(e:React.KeyboardEvent<HTMLDivElement>)=>{
    if(e.key!=='ArrowDown'&&e.key!=='ArrowUp')return;
    e.preventDefault();
    const items=Array.from(containerRef.current?.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]')??[]);
    const i=items.indexOf(document.activeElement as HTMLButtonElement);
    items[(i+(e.key==='ArrowDown'?1:-1)+items.length)%items.length]?.focus();
  };
  return <div className={'language-picker'+(open?' is-open':'')} ref={containerRef}>
    <button ref={triggerRef} type="button" className="language-trigger" aria-label="Select language" aria-haspopup="menu" aria-expanded={open} onClick={()=>setOpen(v=>!v)}>
      <Languages size={16} aria-hidden="true"/><span className="language-code">{selected.code}</span><ChevronDown className="language-chevron" size={15} aria-hidden="true"/>
    </button>
    {open&&<div className="language-menu" role="menu" aria-label="Language" onKeyDown={moveFocus}>
      <p className="language-menu-caption">LANGUAGE</p>
      {languageOptions.map(option=><button type="button" key={option.id} role="menuitemradio" aria-checked={value===option.id}
        className={'language-option'+(value===option.id?' active':'')} onClick={()=>{onChange(option.id);setOpen(false);triggerRef.current?.focus();}}>
        <span className="language-flag" aria-hidden="true">{option.flag}</span>
        <span className="language-name">{option.name}</span>
        <span className="language-option-code">{option.code}</span>
        {value===option.id&&<Check size={15} className="language-check" aria-hidden="true"/>}
      </button>)}
    </div>}
  </div>;
}

function App() {
  const [status,setStatus] = useState<Status>({exists:true,unlocked:false});
  const [snap,setSnap] = useState<Snapshot>(defaultSnap);
  const [theme,setTheme] = useState<'dark'|'light'>(()=>localStorage.getItem('masskey-theme')==='light'?'light':'dark');
  const [lang,setLang] = useState<Language>(()=>{const v=localStorage.getItem('masskey-language');return v==='en'||v==='th'?v:'vi';});
  const t = (text:string) => translate(lang,text);
  const [tab,setTab] = useState<Tab>('all');
  const [filter,setFilter] = useState('');
  const [categoryId,setCategoryId] = useState('');
  const [modal,setModal] = useState<'account'|'category'|'detail'|'recovery'|'import'|'export'|'none'>('none');
  const [draft,setDraft] = useState<AccountDraft>(blank);
  const [categoryDraft,setCategoryDraft] = useState<{id?:string,name:string,icon:string,color:string}>({name:'',icon:'◈',color:colors[0]});
  const [selected,setSelected] = useState<Account|null>(null);
  const [shownPassword,setShownPassword] = useState('');
  const [revealed,setRevealed] = useState(false);
  const [master,setMaster] = useState('');
  const [masterConfirm,setMasterConfirm] = useState('');
  const [recoveryValue,setRecoveryValue] = useState('');
  const [showRecoverySetup,setShowRecoverySetup] = useState(false);
  const [recoveryConfirmed,setRecoveryConfirmed] = useState(false);
  const [recoverMode,setRecoverMode] = useState(false);
  const [secret,setSecret] = useState('');
  const [newPass,setNewPass] = useState('');
  const [zipPath,setZipPath] = useState('');
  const [zipPassword,setZipPassword] = useState('');
  const [zipPasswordConfirm,setZipPasswordConfirm] = useState('');
  const [sourcePass,setSourcePass] = useState('');
  const [importMode,setImportMode] = useState<'merge'|'replace'>('merge');
  const [importRecovery,setImportRecovery] = useState(false);
  const [importNewPass,setImportNewPass] = useState('');
  const [settingsLock,setSettingsLock] = useState(180);
  const [settingsClip,setSettingsClip] = useState(25);
  const [settingsMin,setSettingsMin] = useState(false);
  const [settingsReauth,setSettingsReauth] = useState(false);
  const [settingsWarn,setSettingsWarn] = useState(true);
  const [settingsHealth,setSettingsHealth] = useState(true);
  const [health,setHealth] = useState<HealthReport|null>(null);
  const [reauthInput,setReauthInput] = useState('');
  const [generatorLength,setGeneratorLength] = useState(24);
  const [showWeakWarning,setShowWeakWarning] = useState(true);
  const [showDraftPassword,setShowDraftPassword] = useState(false);
  const [generatedDraft,setGeneratedDraft] = useState(false);
  const [rotateMaster,setRotateMaster] = useState('');
  const [rotating,setRotating] = useState(false);
  const iconInput = useRef<HTMLInputElement>(null);
  const iconTarget = useRef<'account'|'category'>('account');
  const [oldPassword,setOldPassword] = useState('');
  const [freshPassword,setFreshPassword] = useState('');
  const [freshConfirm,setFreshConfirm] = useState('');
  const [notice,setNotice] = useState('');
  const [error,setError] = useState('');
  const [busy,setBusy] = useState(false);
  const [dragging,setDragging] = useState(false);
  const [copied,setCopied] = useState(false);
  const clipTimer = useRef<ReturnType<typeof setTimeout>|null>(null);
  const lastTouch = useRef(0);

  const alert = useCallback((msg:string, failure=false) => {if(failure){setError(msg);setNotice('');}else{setNotice(msg);setError('');}},[]);
  const doWork = useCallback(async (fn:()=>Promise<void>) => {
    if(busy)return; setBusy(true);setError('');
    try {await fn();}catch(e){alert(errorText(e),true);}finally{setBusy(false);}
  },[alert,busy]);
  const load = useCallback(async () => {
    const st = await api<Status>('vault_status');
    setStatus(st);
    if(st.unlocked) {
      const data=await api<Snapshot>('snapshot');setSnap(data);
      setSettingsLock(data.settings.auto_lock_seconds);setSettingsClip(data.settings.clipboard_seconds);
      setSettingsMin(!!data.settings.lock_on_minimize);setSettingsReauth(!!data.settings.require_reauth);
      setSettingsWarn(data.settings.warn_weak_passwords!==false);setSettingsHealth(data.settings.vault_health!==false);
      if(data.settings.vault_health!==false) api<HealthReport>('vault_health').then(setHealth).catch(()=>setHealth(null)); else setHealth(null);
    } else { setSnap(defaultSnap);setHealth(null);setReauthInput('');setShownPassword('');setSelected(null);setTab('all');setModal(v=>v==='import'?v:'none'); }
  },[]);
  useEffect(()=>{load().catch(e=>alert(errorText(e),true));},[load,alert]);
  useEffect(()=>{document.documentElement.dataset.theme=theme;localStorage.setItem('masskey-theme',theme);},[theme]);
  useEffect(()=>{document.documentElement.lang=lang;localStorage.setItem('masskey-language',lang);},[lang]);
  useEffect(()=>{let unsubscribe:undefined|(()=>void);getCurrentWindow().onResized(async()=>{
    if(!status.unlocked||!snap.settings.lock_on_minimize)return;
    try {if(await getCurrentWindow().isMinimized()){await api('lock_vault');await load();}}catch{}
  }).then(fn=>unsubscribe=fn).catch(()=>{});return()=>unsubscribe?.();},[status.unlocked,snap.settings.lock_on_minimize,load]);
  useEffect(()=>{
    if(modal==='none') {
      setZipPassword('');setZipPasswordConfirm('');setSourcePass('');
      setImportNewPass('');setShownPassword('');setReauthInput('');setRevealed(false);
    }
  },[modal]);
  useEffect(()=>{
    const timer=window.setInterval(()=>{api<Status>('vault_status').then(s=>{
      if(status.unlocked && !s.unlocked) {setModal('none');setShownPassword('');setSelected(null);setSnap(defaultSnap);setNotice('Kho đã tự khóa sau thời gian không hoạt động.');}
      setStatus(s);
    }).catch(()=>{});},1800);
    return ()=>clearInterval(timer);
  },[status.unlocked]);
  useEffect(()=>{
    const handler=()=>{
      if(!status.unlocked || Date.now()-lastTouch.current<1000)return;
      lastTouch.current=Date.now(); api('touch_session').catch(()=>{});
    };
    window.addEventListener('pointerdown',handler); window.addEventListener('keydown',handler);
    return ()=>{window.removeEventListener('pointerdown',handler);window.removeEventListener('keydown',handler);};
  },[status.unlocked]);
  useEffect(()=>{
    let alive=true;let unlisten:(()=>void)|undefined;
    getCurrentWebview().onDragDropEvent(e=>{
      if(e.payload.type==='over' || e.payload.type==='enter') setDragging(true);
      if(e.payload.type==='leave')setDragging(false);
      if(e.payload.type==='drop') {
        setDragging(false); const path=e.payload.paths.find(p=>p.toLowerCase().endsWith('.zip'));
        if(path){setZipPath(path);setZipPassword('');setSourcePass('');setModal('import');setTab('backup');setError('');}
        else alert('Chỉ hỗ trợ kéo thả file .zip đã mã hóa.',true);
      }
    }).then(fn=>{if(alive)unlisten=fn;else fn();}).catch(()=>{});
    return ()=>{alive=false;unlisten?.();};
  },[alert]);
  useEffect(()=>{
    const escape=(e:KeyboardEvent)=>{if(e.key==='Escape' && !showRecoverySetup){setModal('none');setShownPassword('');setRevealed(false);}};
    window.addEventListener('keydown',escape);return()=>window.removeEventListener('keydown',escape);
  },[showRecoverySetup]);
  useEffect(()=>()=>{if(clipTimer.current)clearTimeout(clipTimer.current);},[]);

  const copySecret=async(text:string)=>{
    try {
      await navigator.clipboard.writeText(text);setCopied(true);setTimeout(()=>setCopied(false),1600);
      if(clipTimer.current)clearTimeout(clipTimer.current);
      clipTimer.current=setTimeout(async()=>{
        // Never clear an unrelated clipboard item. If read is forbidden, leave it alone.
        try {if(await navigator.clipboard.readText()===text)await navigator.clipboard.writeText('');}catch{}
      },snap.settings.clipboard_seconds*1000);
      return true;
    }catch{alert('Không thể sao chép. Hãy kiểm tra quyền clipboard.',true);return false;}
  };
  const setup=()=>doWork(async()=>{
    if(master!==masterConfirm)throw new Error('Mật khẩu xác nhận không khớp.');
    const value=await api<string>('create_vault',{masterPassword:master});
    setMaster('');setMasterConfirm('');setRecoveryValue(value);setRecoveryConfirmed(false);setShowRecoverySetup(true);
    await load();
  });
  const unlock=()=>doWork(async()=>{
    await api('unlock_vault',{masterPassword:master});setMaster('');setRecoverMode(false);
    await load();alert('Đã mở khóa MassKey Vault.');
  });
  const recover=()=>doWork(async()=>{
    await api('recover_vault',{recoveryKey:secret,newMasterPassword:newPass});
    setSecret('');setNewPass('');setRecoverMode(false);await load();alert('Khôi phục thành công.');
  });
  const lock=()=>doWork(async()=>{await api('lock_vault');setModal('none');setSelected(null);setShownPassword('');await load();});
  const showDetail=(account:Account)=>{setSelected(account);setShownPassword('');setReauthInput('');setRevealed(false);setModal('detail');};
  const editAccount=async(account:Account|null)=>{
    setShownPassword('');setSelected(null);
    setDraft(account?{...account,password:''}:{...blank,categoryId:categoryId||snap.categories[0]?.id||''});
    setShowWeakWarning(true);setShowDraftPassword(false);setGeneratedDraft(false);setModal('account');
  };
  const persistAccount=()=>doWork(async()=>{
    await api('upsert_account',{account:{...draft,password:draft.id && !draft.password ? null : draft.password}});
    setDraft(blank);setShowDraftPassword(false);setGeneratedDraft(false);setModal('none');await load();alert('Đã lưu tài khoản an toàn.');
  });
  const deleteAccount=(account:Account)=>{
    if(!window.confirm(`Xóa tài khoản “${account.title}”? Hành động này không thể hoàn tác.`))return;
    doWork(async()=>{await api('delete_account',{id:account.id});setModal('none');await load();alert('Đã xóa tài khoản.');});
  };
  const editCategory=(category?:Category)=>{setCategoryDraft(category?{...category}:{name:'',icon:'◈',color:colors[0]});setModal('category');};
  const persistCategory=()=>doWork(async()=>{
    await api('upsert_category',categoryDraft);setModal('none');await load();alert('Đã lưu thư viện.');
  });
  const deleteCategory=(id:string)=>{
    if(!window.confirm('Xóa thư viện này? Tài khoản bên trong phải được chuyển trước.'))return;
    doWork(async()=>{await api('delete_category',{id});setModal('none');setTab('all');await load();});
  };
  const reveal=()=>doWork(async()=>{
    if(!selected)return;
    if(revealed){setShownPassword('');setRevealed(false);return;}
    const pass=await api<string>('get_password',{accountId:selected.id,reauthPassword:reauthInput});setReauthInput('');setShownPassword(pass);setRevealed(true);
  });
  const openImportPicker=async()=>{
    try {const path=await open({multiple:false,filters:[{name:'Encrypted ZIP backup',extensions:['zip']} ]});
      if(typeof path==='string'){setZipPath(path);setZipPassword('');setSourcePass('');setModal('import');setError('');}
    }catch(e){alert(errorText(e),true);}
  };
  const exportBackup=()=>doWork(async()=>{
    if(zipPassword!==zipPasswordConfirm)throw new Error('Mật khẩu ZIP xác nhận không khớp.');
    const destination=await save({defaultPath:'MassKey-Backup.zip',filters:[{name:'ZIP',extensions:['zip']}]});
    if(!destination)return;
    await api('export_zip',{destination,zipPassword});setZipPassword('');setZipPasswordConfirm('');setModal('none');
    alert('Đã xuất bản sao ZIP mã hóa AES-256.');
  });
  const importBackup=()=>doWork(async()=>{
    if(importRecovery && (!status.exists || importMode==='replace') && importNewPass.length<12)throw new Error('Cần mật khẩu chính mới, tối thiểu 12 ký tự.');
    if(status.exists && !status.unlocked)throw new Error('Mở khóa vault hiện tại trước khi khôi phục dữ liệu.');
    if(importMode==='replace' && status.exists && !window.confirm('THAY THẾ toàn bộ kho hiện tại? Bản cũ chỉ còn trong file backup của bạn.'))return;
    const count=await api<number>('import_zip',{source:zipPath,zipPassword,sourceSecret:sourcePass,
      recoveryMode:importRecovery,newMasterPassword:importRecovery?importNewPass:null,mode:status.exists?importMode:'replace'});
    setZipPassword('');setSourcePass('');setImportNewPass('');setModal('none');await load();
    alert(`Khôi phục thành công. Bản sao có ${count} tài khoản.`);
  });
  const saveSettings=()=>doWork(async()=>{await api('update_settings',{autoLockSeconds:settingsLock,clipboardSeconds:settingsClip,lockOnMinimize:settingsMin,requireReauth:settingsReauth,warnWeakPasswords:settingsWarn,vaultHealth:settingsHealth});await load();alert('Đã lưu thiết lập bảo mật.');});
  const changeMaster=()=>doWork(async()=>{
    if(freshPassword!==freshConfirm)throw new Error('Mật khẩu mới không trùng khớp.');
    await api('change_master',{currentPassword:oldPassword,newPassword:freshPassword});
    setOldPassword('');setFreshPassword('');setFreshConfirm('');alert('Đã đổi Master Password. Recovery Key vẫn giữ nguyên.');
  });
  const createPassword=(similar=false)=>doWork(async()=>{
    const p=similar ? await api<string>('generate_similar_password',{original:draft.password}) : await api<string>('generate_password',{length:generatorLength});
    setDraft(d=>({...d,password:p}));setShowWeakWarning(false);setShowDraftPassword(false);setGeneratedDraft(true);
    alert(t('Đổi mật khẩu trên website trước khi cập nhật vault.'));
  });
  const rotateRecovery=()=>doWork(async()=>{
    const recovery=await api<string>('rotate_recovery',{masterPassword:rotateMaster});
    setRotateMaster('');setRotating(false);setRecoveryValue(recovery);setRecoveryConfirmed(false);setShowRecoverySetup(true);
  });
  const openWebsite=(account:Account)=>doWork(async()=>{
    await api('open_account_website',{accountId:account.id});
  });
  const chooseIcon=(target:'account'|'category')=>{iconTarget.current=target;iconInput.current?.click();};
  const onIconFile=async(e:React.ChangeEvent<HTMLInputElement>)=>{
    const file=e.target.files?.[0];e.target.value='';if(!file)return;
    if(file.size>2*1024*1024 || !['image/png','image/jpeg','image/webp','image/x-icon','image/vnd.microsoft.icon'].includes(file.type) && !/\.(ico|png|jpg|jpeg|webp)$/i.test(file.name)){
      alert('Please choose a PNG, JPG, WEBP or ICO under 2 MB.',true);return;
    }
    const objectUrl=URL.createObjectURL(file);
    try {
      const image=new Image();image.src=objectUrl;await image.decode();
      if(image.naturalWidth>4096||image.naturalHeight>4096)throw Error('Image is too large');
      const canvas=document.createElement('canvas');canvas.width=80;canvas.height=80;
      const ctx=canvas.getContext('2d');if(!ctx)throw Error('Image decoding failed');
      ctx.clearRect(0,0,80,80);
      const scale=Math.min(80/image.naturalWidth,80/image.naturalHeight);
      const w=image.naturalWidth*scale,h=image.naturalHeight*scale;
      ctx.drawImage(image,(80-w)/2,(80-h)/2,w,h);
      const value=canvas.toDataURL('image/png');
      if(value.length>89000)throw Error('Icon is too large after processing');
      if(iconTarget.current==='account')setDraft(d=>({...d,icon:value}));
      else setCategoryDraft(d=>({...d,icon:value}));
    }catch(err){alert(errorText(err),true);}finally{URL.revokeObjectURL(objectUrl);}
  };
  const iconView=(value:string|undefined, fallback='◈')=>value?.startsWith('data:image/png;base64,')?<img className="user-icon" src={value} alt=""/>:value||fallback;
  const weakDraft=useMemo(()=>{
    const p=draft.password;if(!p)return false;
    const common=/password|qwerty|123456|admin|letmein|abcdef|iloveyou/i.test(p);
    const kinds=Number(/[a-z]/.test(p))+Number(/[A-Z]/.test(p))+Number(/[0-9]/.test(p))+Number(/[^a-z0-9]/i.test(p));
    return p.length<12 || common || (p.length<16 && kinds<=2) || new Set(p).size<=3;
  },[draft.password]);

  const visibleAccounts=useMemo(()=>snap.accounts.filter(a=>{
    if(tab==='category' && a.categoryId!==categoryId)return false;
    const q=filter.trim().toLocaleLowerCase();
    return !q || [a.title,a.username,a.website].join(' ').toLocaleLowerCase().includes(q);
  }),[snap.accounts,tab,categoryId,filter]);
  const selectedCategory=snap.categories.find(c=>c.id===categoryId);
  const languagePicker=<LanguageMenu value={lang} onChange={setLang}/>;
  const themeButton=<button className="icon-btn mode-toggle" title={theme==='dark'?t("Bật giao diện sáng"):t("Bật giao diện tối")} onClick={()=>setTheme(v=>v==='dark'?'light':'dark')}>{theme==='dark'?<Sun size={19}/>:<Moon size={19}/>}</button>;
  return <div className="app-shell"><input type="file" hidden accept=".png,.jpg,.jpeg,.webp,.ico,image/png,image/jpeg,image/webp,image/x-icon" ref={iconInput} onChange={onIconFile}/>
    {status.unlocked && !showRecoverySetup && <aside className="sidebar">
      <div className="sidebar-brand"><div className="brand-mark"><img src={logo} alt="MassKey Vault" /></div><div><div className="brand-text">MASSKEY <span>VAULT</span></div><div className="brand-sub">YOUR PRIVATE SPACE</div></div></div>
      <div className="nav-block"><p className="nav-caption">{t("THƯ VIỆN")}</p>
        <button className={'nav-item '+(tab==='all'?'active':'')} onClick={()=>{setTab('all');setFilter('');}}><LayoutGrid size={17}/><span>{t("Tất cả tài khoản")}</span><span className="nav-count">{snap.accounts.length}</span></button>
        <div className="nav-categories">{snap.categories.map(c=><button key={c.id} className={'nav-item '+(tab==='category'&&c.id===categoryId?'active':'')} onClick={()=>{setTab('category');setCategoryId(c.id);setFilter('');}}><span className="category-symbol" style={{color:c.color}}>{iconView(c.icon)}</span><span>{c.name}</span><span className="nav-count">{snap.accounts.filter(a=>a.categoryId===c.id).length}</span></button>)}</div>
        <button className="nav-item nav-add" onClick={()=>editCategory()}><FolderPlus size={17}/><span>{t("Thêm thư viện")}</span><Plus size={15}/></button>
      </div>
      <div className="sidebar-bottom"><p className="nav-caption">{t("KHÔNG GIAN")}</p>
        <button className={'nav-item '+(tab==='backup'?'active':'')} onClick={()=>setTab('backup')}><ArchiveRestore size={18}/>{t("Sao lưu & khôi phục")}</button>
        <button className={'nav-item '+(tab==='settings'?'active':'')} onClick={()=>setTab('settings')}><Settings2 size={18}/>{t("Bảo mật & cài đặt")}</button>
        <div className="security-card"><div className="secure-title"><span className="pulse-dot"/>{t("Kho đang mở khóa")}</div><small>{t("Mã hóa đầu cuối · Lưu cục bộ")}</small></div>
        <button className="lock-btn" onClick={lock}><Lock size={16}/>{t("Khóa ngay")}</button>
      </div>
    </aside>}
    <main className={'main-area '+(!status.unlocked?'main-auth':'')}>
      <header className="topbar"><div className="topbar-left">{status.unlocked?<><span className="breadcrumb">MassKey</span><span className="breadcrumb-divider">/</span><strong>{tab==='settings'?t("Bảo mật"):tab==='backup'?t("Sao lưu"):tab==='category'?selectedCategory?.name:t("My vault")}</strong></>:<><img className="small-logo" src={logo} alt=""/><strong>MassKey Vault</strong></>}</div><div className="topbar-right"><span className="local-tag"><span className="local-dot"/>OFFLINE VAULT</span>{languagePicker}{themeButton}</div></header>
      {(!status.unlocked) ? <section className="auth-screen"><div className="auth-aurora"/><div className="auth-content"><img src={logo} className="auth-logo" alt="MassKey Vault"/><p className="eyebrow">YOUR SECRETS. YOUR CONTROL.</p><h1>{!status.exists?t("Your vault starts here."):recoverMode?t("Recover your vault."):t("Welcome back.")}</h1><p className="auth-desc">{!status.exists?t("Tạo một không gian riêng tư chỉ bạn có thể mở."):t("Mọi bí mật của bạn đều được mã hóa trên thiết bị này.")}</p>
        <div className="auth-form">
          {recoverMode ? <><label>{t("RECOVERY KEY")}</label><textarea value={secret} onChange={e=>setSecret(e.target.value)} placeholder={t("Nhập Recovery Key đã lưu")} rows={3}/><label>{t("MASTER PASSWORD MỚI")}</label><input type="password" autoComplete="new-password" value={newPass} onChange={e=>setNewPass(e.target.value)} placeholder="Ít nhất 12 ký tự"/><button className="primary wide" disabled={busy} onClick={recover}>{t("Khôi phục quyền truy cập")} <ArrowRight size={17}/></button><button className="plain-link" onClick={()=>setRecoverMode(false)}>{t("Quay lại mở khóa")}</button></>
          : <><label>{t("MASTER PASSWORD")}</label><input type="password" autoFocus autoComplete={status.exists?'current-password':'new-password'} value={master} onChange={e=>setMaster(e.target.value)} onKeyDown={e=>{if(e.key==='Enter'&&status.exists)unlock();}} placeholder={t("Nhập mật khẩu chính")}/>
            {!status.exists && <><label>{t("XÁC NHẬN MẬT KHẨU")}</label><input type="password" autoComplete="new-password" value={masterConfirm} onChange={e=>setMasterConfirm(e.target.value)} placeholder={t("Nhập lại mật khẩu chính")}/></>}
            <button className="primary wide" disabled={busy} onClick={status.exists?unlock:setup}>{busy?t("Đang xử lý…"):status.exists?t("Mở khóa kho"):t("Tạo vault an toàn")} <ArrowRight size={17}/></button>
            {status.exists ? <button className="plain-link" onClick={()=>setRecoverMode(true)}>{t("Quên mật khẩu? Dùng Recovery Key")}</button> : <div className="auth-foot"><ShieldCheck size={16}/>{t("Mật khẩu chính phải có tối thiểu 12 ký tự")}</div>}
          </>}
          {!status.exists && <button className="outline wide auth-import" onClick={openImportPicker}><UploadCloud size={17}/>{t("Tôi có bản sao ZIP — Khôi phục")}</button>}
        </div><div className="auth-proof"><LockKeyhole size={15}/>Zero cloud · Local encryption · Private by design</div>
      </div></section>
      : <div className="scroll-area">
        {tab==='all'||tab==='category' ? <div className="workspace animate-enter">
          <div className="page-header"><div><p className="eyebrow">PERSONAL PASSWORD MANAGER</p><h1>{tab==='category'?selectedCategory?.name||t("Thư viện"):t("My vault")}<span className="header-dot">.</span></h1><p className="description">{t("Không gian bảo mật dành riêng cho những tài khoản quan trọng.")}</p></div><div className="header-actions">{tab==='category'&&<button title={t("Chỉnh sửa thư viện")} className="btn-outline" onClick={()=>selectedCategory&&editCategory(selectedCategory)}><Pencil size={15}/>{t("Sửa thư viện")}</button>}<button className="primary" onClick={()=>editAccount(null)}><Plus size={17}/>{t("Thêm tài khoản")}</button></div></div>
          <div className="hero-strip"><div className="hero-icon"><ShieldCheck size={27}/></div><div className="hero-body"><p>{t("ENCRYPTED COLLECTION")}</p><strong>{snap.accounts.length.toString().padStart(2,'0')} <span>{t("tài khoản được bảo vệ")}</span></strong><small>Master password · Argon2id · XChaCha20-Poly1305</small></div><span className="hero-seal"><span className="pulse-dot"/>{t("SECURED LOCALLY")}</span></div>
          <div className="tool-row"><div className="search-wrap"><Search size={19}/><input value={filter} onChange={e=>setFilter(e.target.value)} placeholder={t("Tìm tên dịch vụ, email hoặc website...")}/><span className="search-key">SEARCH</span></div><div className="result-count"><SlidersHorizontal size={17}/>{visibleAccounts.length} {t("tài khoản")}</div></div>
          <div className="section-heading"><h2>{t("TÀI KHOẢN CỦA BẠN")}</h2><span>{tab==='category'?selectedCategory?.name:t("Tất cả danh mục")}</span></div>
          {visibleAccounts.length ? <div className="account-grid">{visibleAccounts.map((a,i)=>{
            const c=snap.categories.find(cat=>cat.id===a.categoryId);
            return <button className="account-card" style={{animationDelay:`${Math.min(i,10)*25}ms`}} key={a.id} onClick={()=>showDetail(a)}>
              <span className="account-avatar" style={{'--avatar-color':c?.color||'#9074e8'} as React.CSSProperties}>{iconView(a.icon||c?.icon)}</span>
              <span className="account-copy"><strong>{a.title}</strong><small>{a.username||t("Chưa có username")}</small><span className="account-category">{c?.name||t("Chưa phân loại")}</span></span><span className="account-arrow"><ArrowRight size={18}/></span>
            </button>})}</div> : <div className="empty-state"><span className="empty-art"><FolderClosed size={33}/></span><h3>{filter?t("Không tìm thấy tài khoản"):t("Chưa có tài khoản nào")}</h3><p>{filter?t("Thử một từ khóa khác."):t("Tạo tài khoản đầu tiên để bắt đầu quản lý.")}</p><button className="btn-outline" onClick={()=>editAccount(null)}><Plus size={16}/>{t("Thêm tài khoản")}</button></div>}
        </div> : tab==='backup' ? <div className="workspace animate-enter narrow"><p className="eyebrow">{t("YOUR DATA, YOUR CONTROL")}</p><h1>{t("Sao lưu & khôi phục")}<span className="header-dot">.</span></h1><p className="description">{t("Di chuyển an toàn sang máy mới mà không cần dịch vụ đám mây.")}</p>
          <div className="action-panel"><div className="panel-icon violet"><ArrowDownToLine size={23}/></div><div><h2>{t("Xuất ZIP mã hóa")}</h2><p>{t("Tạo file ZIP sử dụng AES-256 với mật khẩu riêng. Dữ liệu bên trong vẫn được mã hóa bởi Master Password.")}</p></div><button className="primary" onClick={()=>{setZipPassword('');setZipPasswordConfirm('');setModal('export');}}>{t("Xuất backup")} <ArrowRight size={16}/></button></div>
          <div className="action-panel"><div className="panel-icon teal"><UploadCloud size={24}/></div><div><h2>{t("Khôi phục bằng kéo thả")}</h2><p>{t("Thả file .zip vào cửa sổ ứng dụng, hoặc chọn file. Không cần giải nén thủ công.")}</p></div><button className="btn-outline" onClick={openImportPicker}>{t("Chọn file ZIP")} <ArrowRight size={16}/></button></div>
          <div className="notice-panel"><ShieldCheck size={20}/><p>{t("Để khôi phục, bạn cần mật khẩu ZIP và Master Password của vault nguồn (hoặc Recovery Key). Mật khẩu ZIP không thay thế lớp mã hóa bên trong.")}</p></div>
        </div> : <div className="workspace animate-enter narrow"><p className="eyebrow">{t("PROTECTION & PREFERENCES")}</p><h1>{t("Security center")}<span className="header-dot">.</span></h1><p className="description">{t("Quản lý an toàn dữ liệu và quyền truy cập vault.")}</p>
          <section className="settings-section"><div className="settings-section-title"><Clock3 size={20}/><h2>{t("Tự động khóa")}</h2></div><p>{t("Vault tự khóa khi không có tương tác. Thời gian tối đa: 60 phút.")}</p><div className="settings-row"><label>{t("Thời gian chờ")}</label><select value={settingsLock} onChange={e=>setSettingsLock(Number(e.target.value))}>{[30,60,180,300,600,900,1800,3600].map(n=><option key={n} value={n}>{n<60?`${n} ${t("giây")}`:`${n/60} ${t("phút")}`}</option>)}</select></div><div className="settings-row"><label>{t("Tự xóa clipboard")}</label><select value={settingsClip} onChange={e=>setSettingsClip(Number(e.target.value))}>{[5,10,15,25,30,60,120].map(n=><option key={n} value={n}>{n} {t("giây")}</option>)}</select></div><div className="toggle-settings">
          <label><span>{t('Khóa khi thu nhỏ')}</span><input type="checkbox" checked={settingsMin} onChange={e=>setSettingsMin(e.target.checked)}/></label>
          <label><span>{t('Yêu cầu Master Password khi xem/copy')}</span><input type="checkbox" checked={settingsReauth} onChange={e=>setSettingsReauth(e.target.checked)}/></label>
          <label><span>{t('Cảnh báo mật khẩu yếu')}</span><input type="checkbox" checked={settingsWarn} onChange={e=>setSettingsWarn(e.target.checked)}/></label>
          <label><span>{t('Vault Health')}</span><input type="checkbox" checked={settingsHealth} onChange={e=>setSettingsHealth(e.target.checked)}/></label>
          </div><button className="primary" onClick={saveSettings} disabled={busy}>{t('Lưu thiết lập')}</button></section>
          <section className="settings-section"><div className="settings-section-title"><Languages size={20}/><h2>{t('GIAO DIỆN')}</h2></div><div className="settings-row"><label>{t('NGÔN NGỮ')}</label>{languagePicker}</div><div className="settings-row"><label>{t('CHẾ ĐỘ')}</label><button className="btn-outline" onClick={()=>setTheme(v=>v==='dark'?'light':'dark')}>{theme==='dark'?'Dark':'Light'}</button></div></section>
          <section className="settings-section"><div className="settings-section-title"><Activity size={20}/><h2>{t('Vault Health')}</h2></div>{settingsHealth ? <><div className="health-counters"><div><strong>{health?.weak.length??'—'}</strong>{t('Mật khẩu yếu')}</div><div><strong>{health?.reused.length??'—'}</strong>{t('Mật khẩu trùng')}</div></div>{health && (health.weak.length>0||health.reused.length>0)&&<div className="health-list">{snap.accounts.filter(a=>health.weak.includes(a.id)||health.reused.includes(a.id)).map(a=><button key={a.id} className="health-entry" onClick={()=>showDetail(a)}><span>{a.title}</span><small>{health.reused.includes(a.id)?t('Mật khẩu trùng'):t('Mật khẩu yếu')}</small><ArrowRight size={15}/></button>)}</div>}<button className="btn-outline" onClick={()=>api<HealthReport>('vault_health').then(setHealth).catch(e=>alert(errorText(e),true))}>{t('Kiểm tra Vault Health')}</button></>:<p>Vault Health disabled</p>}</section>
          <section className="settings-section"><div className="settings-section-title"><RefreshCw size={20}/><h2>{t('Emergency Recovery Kit')}</h2></div><p>{t('Chỉ có thể lấy Recovery Key mới sau khi xác thực. Cất bản sao ngoại tuyến.')}</p><p>{t('Không có bản sao cũ vẫn khôi phục được?')}</p><button className="btn-outline" onClick={()=>setRotating(v=>!v)}>{t('Đổi Recovery Key')}</button>{rotating&&<div className="field-stack"><p>{t('Tạo mã mới sẽ vô hiệu hóa Recovery Key cũ.')}</p><input type="password" autoComplete="off" placeholder={t('Master Password để tạo mã mới')} value={rotateMaster} onChange={e=>setRotateMaster(e.target.value)}/><button disabled={!rotateMaster||busy} className="primary" onClick={rotateRecovery}>{t('Tạo Recovery Key mới')}</button></div>}</section>
          <section className="settings-section"><div className="settings-section-title"><KeyRound size={20}/><h2>{t("Đổi Master Password")}</h2></div><p>{t("Không thay đổi Recovery Key. Mật khẩu mới phải từ 12 ký tự.")}</p><div className="field-stack"><input type="password" value={oldPassword} onChange={e=>setOldPassword(e.target.value)} placeholder={t("Mật khẩu hiện tại")}/><input type="password" autoComplete="new-password" value={freshPassword} onChange={e=>setFreshPassword(e.target.value)} placeholder={t("Mật khẩu mới (ít nhất 12 ký tự)")}/><input type="password" autoComplete="new-password" value={freshConfirm} onChange={e=>setFreshConfirm(e.target.value)} placeholder={t("Xác nhận mật khẩu mới")}/></div><button className="btn-outline" disabled={busy} onClick={changeMaster}>{t("Cập nhật mật khẩu")}</button></section>
          <div className="notice-panel red"><ShieldOff size={18}/><p>{t("Ứng dụng chưa qua kiểm toán bảo mật độc lập. Malware có quyền kiểm soát máy có thể đọc dữ liệu khi vault mở. Không lưu mật khẩu quan trọng trước khi kiểm thử bản build.")}</p></div>
        </div>}
      </div>}
      {(notice||error) && <div role="status" className={'toast '+(error?'toast-error':'')}><span>{error||notice}</span><button onClick={()=>{setError('');setNotice('');}} aria-label="Đóng"><X size={16}/></button></div>}
    </main>
    {dragging&&<div className="drop-overlay"><UploadCloud size={48}/><h2>{t("Thả bản sao ZIP mã hóa vào đây")}</h2><span>MASSKEY VAULT · SECURE RESTORE</span></div>}
    {showRecoverySetup && <div className="modal-scrim"><div className="modal-box recovery-modal"><div className="modal-icon"><ShieldCheck size={26}/></div><p className="eyebrow">RECOVERY ACCESS</p><h2>{t("Lưu Recovery Key của bạn.")}</h2><p className="muted">{t("Đây là cơ hội duy nhất để sao chép mã khôi phục. Ai có mã này có thể truy cập vault, hãy cất giữ ngoài thiết bị và không chia sẻ.")}</p><div className="recovery-code">{recoveryValue}</div><button className="btn-outline wide" onClick={()=>copySecret(recoveryValue)}><ClipboardCopy size={17}/>{copied?t('Đã sao chép'):t('Sao chép Recovery Key')}</button><label className="check-row"><input type="checkbox" checked={recoveryConfirmed} onChange={e=>setRecoveryConfirmed(e.target.checked)}/>{t("Tôi đã lưu Recovery Key ở nơi an toàn.")}</label><button className="primary wide" disabled={!recoveryConfirmed} onClick={()=>{setRecoveryValue('');setShowRecoverySetup(false);}}>{t("Tiếp tục vào vault")} <ArrowRight size={17}/></button></div></div>}
    {modal!=='none' && !showRecoverySetup && <div className="modal-scrim" onMouseDown={e=>{if(e.target===e.currentTarget){setModal('none');setShownPassword('');setDraft(blank);setShowDraftPassword(false);setGeneratedDraft(false);}}}><section className={'modal-box '+(['account','detail'].includes(modal)?'account-modal':'')}><header className="modal-heading"><div><p className="eyebrow">MASSKEY VAULT</p><h2>{modal==='account'?(draft.id?t("Chỉnh sửa tài khoản"):t("Tài khoản mới")):modal==='category'?(categoryDraft.id?t("Chỉnh sửa thư viện"):t("Tạo thư viện mới")):modal==='detail'?selected?.title:modal==='export'?t("Xuất backup mã hóa"):t("Khôi phục dữ liệu")}</h2></div><button className="icon-btn" onClick={()=>{setModal('none');setShownPassword('');setDraft(blank);setShowDraftPassword(false);setGeneratedDraft(false);}} title="Đóng"><X size={20}/></button></header>
      {modal==='account' && <div className="modal-body"><div className="field"><label>{t("TÊN DỊCH VỤ *")}</label><input value={draft.title} onChange={e=>setDraft({...draft,title:e.target.value})} placeholder={t("Ví dụ: GitHub, Steam, Ngân hàng...")}/></div>
        <div className="field-row"><div className="field"><label>{t("THƯ VIỆN")}</label><select value={draft.categoryId} onChange={e=>setDraft({...draft,categoryId:e.target.value})}><option value="">{t("Chưa phân loại")}</option>{snap.categories.map(c=><option key={c.id} value={c.id}>{c.icon.startsWith('data:')?'▣':c.icon} {c.name}</option>)}</select></div><div className="field"><label>{t("ICON")}</label><div className="inline-icon-picker"><select value={draft.icon.startsWith('data:')?'CUSTOM':draft.icon} onChange={e=>setDraft({...draft,icon:e.target.value})}>{draft.icon.startsWith('data:')&&<option value="CUSTOM">{t("Custom image")}</option>}{icons.map(i=><option key={i} value={i}>{i}</option>)}</select><button className="btn-outline" title={t('Chọn icon từ máy')} onClick={()=>chooseIcon('account')}><ImagePlus size={16}/></button>{draft.icon.startsWith('data:')&&<span className="icon-preview">{iconView(draft.icon)}</span>}</div></div></div>
        <div className="field"><label>{t("EMAIL / TÊN ĐĂNG NHẬP")}</label><input autoComplete="off" value={draft.username} onChange={e=>setDraft({...draft,username:e.target.value})} placeholder={t("Username hoặc email")}/></div>
        <div className="field"><label>{t('MẬT KHẨU')} {draft.id&&<span className="optional">{t("(Để trống nếu không thay đổi)")}</span>}</label><div className="password-input-group"><input type={showDraftPassword?'text':'password'} autoComplete="new-password" spellCheck={false} value={draft.password} onChange={e=>{setDraft({...draft,password:e.target.value});setGeneratedDraft(false);}} placeholder={draft.id?t('Giữ mật khẩu hiện tại'):t('Nhập hoặc tạo mật khẩu')}/><div className="password-input-actions"><button type="button" className="btn-outline password-action" disabled={!draft.password} onClick={()=>setShowDraftPassword(v=>!v)} title={showDraftPassword?t('Ẩn mật khẩu'):t('Hiện mật khẩu')} aria-label={showDraftPassword?t('Ẩn mật khẩu'):t('Hiện mật khẩu')} aria-pressed={showDraftPassword}>{showDraftPassword?<EyeOff size={18}/>:<Eye size={18}/>}</button><button type="button" className="btn-outline password-action" disabled={!draft.password} onClick={async()=>{if(await copySecret(draft.password))alert(t('Đã sao chép mật khẩu'));}} title={t('Sao chép mật khẩu')} aria-label={t('Sao chép mật khẩu')}><ClipboardCopy size={17}/></button><button type="button" className="btn-outline password-action action-generate" disabled={busy} onClick={()=>createPassword(false)} title={t('Tạo mật khẩu mạnh mới')} aria-label={t('Tạo mật khẩu mạnh mới')}><Zap size={17}/></button></div></div>{generatedDraft&&<p className="password-update-hint">{t('Đổi mật khẩu trên website trước khi cập nhật vault.')}</p>}<div className="password-tools"><label>Length <select value={generatorLength} onChange={e=>setGeneratorLength(Number(e.target.value))}>{[16,20,24,28,32,40,64].map(v=><option value={v} key={v}>{v}</option>)}</select></label></div>
        {snap.settings.warn_weak_passwords!==false && showWeakWarning && weakDraft && <div className="weak-panel"><strong>{t('Mật khẩu hơi yếu, muốn cải thiện không?')}</strong><p>{t('Đổi mật khẩu trên website trước khi cập nhật vault.')}</p><div className="weak-actions"><button className="btn-outline" onClick={()=>setShowWeakWarning(false)}>{t('Giữ nguyên')}</button><button className="btn-outline" onClick={()=>createPassword(true)}>{t('Cải thiện từ mật khẩu cũ')}</button><button className="primary" onClick={()=>createPassword(false)}>{t('Tạo mật khẩu mạnh mới')}</button></div></div>}</div>
        <div className="field"><label>{t("WEBSITE")}</label><input value={draft.website} onChange={e=>setDraft({...draft,website:e.target.value})} placeholder="https://example.com"/></div><div className="field"><label>{t("GHI CHÚ / TÀI KHOẢN PHỤ")}</label><textarea rows={3} value={draft.notes} onChange={e=>setDraft({...draft,notes:e.target.value})} placeholder={t("Thông tin bổ sung được mã hóa...")}/></div><div className="form-actions"><button className="btn-outline" onClick={()=>{setModal('none');setDraft(blank);setShowDraftPassword(false);setGeneratedDraft(false);}}>{t("Hủy")}</button><button className="primary" disabled={busy||!draft.title.trim()} onClick={persistAccount}><Check size={17}/>{t("Lưu tài khoản")}</button></div></div>}
      {modal==='category' && <div className="modal-body"><div className="field"><label>{t("TÊN THƯ VIỆN")}</label><input value={categoryDraft.name} onChange={e=>setCategoryDraft({...categoryDraft,name:e.target.value})} placeholder={t("Ví dụ: Game, Công việc, Ngân hàng")}/></div><div className="field"><label>{t("CHỌN ICON")}</label><div className="icon-picker">{icons.map(i=><button key={i} title={i} className={'icon-option '+(categoryDraft.icon===i?'chosen':'')} onClick={()=>setCategoryDraft({...categoryDraft,icon:i})}>{i}</button>)}<button className="icon-option file-icon-option" title={t("Chọn icon từ máy")} onClick={()=>chooseIcon("category")}><ImagePlus size={18}/></button>{categoryDraft.icon.startsWith("data:")&&<span className="icon-preview">{iconView(categoryDraft.icon)}</span>}</div></div><div className="field"><label>{t("MÀU NHẬN DIỆN")}</label><div className="color-picker">{colors.map(c=><button key={c} style={{background:c}} onClick={()=>setCategoryDraft({...categoryDraft,color:c})} className={categoryDraft.color===c?'color-chosen':''} title={c}/>)}</div></div><div className="form-actions">{categoryDraft.id&&<button className="danger-text" onClick={()=>deleteCategory(categoryDraft.id!)}><Trash2 size={15}/>{t("Xóa")}</button>}<button className="primary" disabled={!categoryDraft.name.trim()||busy} onClick={persistCategory}>{t("Lưu thư viện")}</button></div></div>}
      {modal==='detail' && selected && <div className="modal-body"><div className="detail-top"><span className="detail-avatar">{iconView(selected.icon,'✦')}</span><div><h3>{selected.title}</h3><p>{snap.categories.find(c=>c.id===selected.categoryId)?.name||t("Chưa phân loại")}</p></div></div><div className="detail-list">{snap.settings.require_reauth&&<div className="field"><label>{t("Nhập lại Master Password để xem hoặc sao chép")}</label><input type="password" autoComplete="off" value={reauthInput} onChange={e=>setReauthInput(e.target.value)}/></div>}<div><label>{t("TÊN ĐĂNG NHẬP")}</label><div className="detail-value"><span>{selected.username||'—'}</span>{selected.username&&<button className="mini-icon" onClick={()=>copySecret(selected.username)}><ClipboardCopy size={16}/></button>}</div></div><div><label>{t("MẬT KHẨU")}</label><div className="detail-value"><span className="mono-secret">{revealed?shownPassword:'••••••••••••••'}</span><div className="detail-tools"><button onClick={reveal} title="Hiện / Ẩn">{revealed?<EyeOff size={18}/>:<Eye size={18}/>}</button><button title="Sao chép mật khẩu" onClick={()=>doWork(async()=>{const pass=await api<string>('get_password',{accountId:selected.id,reauthPassword:reauthInput});setReauthInput('');await copySecret(pass);})}><ClipboardCopy size={18}/></button></div></div></div><div><label>{t("WEBSITE")}</label><div className="detail-value"><span>{selected.website||'—'}</span>{selected.website&&<button className="mini-icon" title="Open in default browser" onClick={()=>openWebsite(selected)}><ExternalLink size={17}/></button>}</div></div><div><label>{t("GHI CHÚ")}</label><div className="detail-value notes-content">{selected.notes||'Không có ghi chú'}</div></div></div><div className="form-actions spaced"><button className="danger-text" onClick={()=>deleteAccount(selected)}><Trash2 size={15}/>{t("Xóa")}</button><button className="primary" onClick={()=>editAccount(selected)}><Pencil size={16}/>{t("Chỉnh sửa")}</button></div></div>}
      {modal==='export' && <div className="modal-body"><div className="notice-panel"><Shield size={20}/><p>{t("ZIP được mã hóa AES-256. File vault bên trong vẫn có lớp mã hóa XChaCha20-Poly1305 riêng.")}</p></div><div className="field"><label>{t("MẬT KHẨU ZIP RIÊNG *")}</label><input type="password" autoComplete="new-password" value={zipPassword} onChange={e=>setZipPassword(e.target.value)} placeholder={t("Tối thiểu 16 ký tự")}/></div><div className="field"><label>{t("XÁC NHẬN MẬT KHẨU ZIP")}</label><input type="password" autoComplete="new-password" value={zipPasswordConfirm} onChange={e=>setZipPasswordConfirm(e.target.value)} placeholder={t("Nhập lại mật khẩu ZIP")}/></div><div className="form-actions"><button className="btn-outline" onClick={()=>setModal('none')}>{t("Hủy")}</button><button className="primary" disabled={busy||zipPassword.length<16} onClick={exportBackup}><ArrowDownToLine size={17}/>{t("Chọn nơi lưu ZIP")}</button></div></div>}
      {modal==='import' && <div className="modal-body"><div className="import-file"><ArchiveRestore size={24}/><span>{zipPath.split(/[\\/]/).pop()}</span><button className="text-link" onClick={openImportPicker}>{t("Thay đổi")}</button></div><div className="field"><label>{t("MẬT KHẨU ZIP")}</label><input type="password" autoComplete="off" value={zipPassword} onChange={e=>setZipPassword(e.target.value)} placeholder={t("Mật khẩu đã đặt khi xuất ZIP")}/></div><label className="check-row"><input type="checkbox" checked={importRecovery} onChange={e=>setImportRecovery(e.target.checked)}/>{t("Dùng Recovery Key thay vì Master Password nguồn")}</label><div className="field"><label>{importRecovery?t('RECOVERY KEY CỦA VAULT NGUỒN'):t('MASTER PASSWORD CỦA VAULT NGUỒN')}</label><input type="password" autoComplete="off" value={sourcePass} onChange={e=>setSourcePass(e.target.value)} placeholder={t("Bí mật để giải mã vault bên trong ZIP")}/></div>{importRecovery&&(!status.exists||importMode==='replace')&&<div className="field"><label>{t("MASTER PASSWORD MỚI")}</label><input type="password" autoComplete="new-password" value={importNewPass} onChange={e=>setImportNewPass(e.target.value)} placeholder={t("Từ 12 ký tự")}/></div>}{status.exists&&<div className="field"><label>{t("PHƯƠNG THỨC NHẬP")}</label><select value={importMode} onChange={e=>setImportMode(e.target.value as 'merge'|'replace')}><option value="merge">{t("Gộp vào vault đang mở (giữ tài khoản cũ)")}</option><option value="replace">{t("Thay thế toàn bộ vault hiện tại")}</option></select></div>}<div className="form-actions"><button className="btn-outline" onClick={()=>setModal('none')}>{t("Hủy")}</button><button className="primary" disabled={busy||!zipPath||!zipPassword||!sourcePass} onClick={importBackup}><ArchiveRestore size={16}/>{t("Khôi phục dữ liệu")}</button></div></div>}
      </section></div>}
  </div>;
}

createRoot(document.getElementById('root')!).render(<React.StrictMode><App/></React.StrictMode>);
