//! MassKey Vault (v1.2 development preview). Security-critical paths require independent audit
//! before storing irreplaceable credentials. Never log secrets.
use std::{fs::{self, File, OpenOptions}, io::{Read, Write}, path::{Path, PathBuf}, sync::{Arc, Mutex}, time::{Duration, Instant}};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{aead::{Aead, KeyInit, Payload}, Key, XChaCha20Poly1305, XNonce};
use rand::{rngs::OsRng, Rng, RngCore};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use uuid::Uuid;
use zeroize::{Zeroize, Zeroizing};
use sha2::{Digest, Sha256};

const VERSION: u32 = 1;
const MEMORY_KIB: u32 = 65536;
const ITERATIONS: u32 = 3;
const PARALLELISM: u32 = 1;
const MAX_VAULT_BYTES: u64 = 24 * 1024 * 1024;
const AAD_MASTER: &[u8] = b"MassKey-v1/master-wrap";
const AAD_RECOVERY: &[u8] = b"MassKey-v1/recovery-wrap";
const AAD_DATA: &[u8] = b"MassKey-v1/vault-data";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxedCipher { nonce: String, ciphertext: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultFile {
    version: u32, salt: String, memory_kib: u32, iterations: u32,
    parallelism: u32, master_wrap: BoxedCipher, recovery_wrap: BoxedCipher,
    payload: BoxedCipher,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Category { id: String, name: String, icon: String, color: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Account {
    id: String, category_id: String, title: String, username: String,
    password: String, website: String, notes: String, icon: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct Resource {
    id: String, url: String, title: String, description: String, notes: String,
    tags: Vec<String>, collection: String, kind: String,
    #[serde(default)] folder_path: String,
    #[serde(default)] docs_url: String,
    #[serde(default)] docs_checked: bool,
    #[serde(default)] stars: Option<u64>,
    #[serde(default)] updated_at: String,
    #[serde(default)] metadata_status: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultSettings {
    auto_lock_seconds: u64, clipboard_seconds: u64,
    #[serde(default)] lock_on_minimize: bool,
    #[serde(default)] require_reauth: bool,
    #[serde(default = "true_value")] warn_weak_passwords: bool,
    #[serde(default = "true_value")] vault_health: bool,
}
fn true_value() -> bool { true }
impl Default for VaultSettings {
    fn default() -> Self { Self { auto_lock_seconds: 180, clipboard_seconds: 25, lock_on_minimize: false, require_reauth: false, warn_weak_passwords: true, vault_health: true } }
}
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct VaultData {
    categories: Vec<Category>, accounts: Vec<Account>, settings: VaultSettings,
    #[serde(default)] trash: Vec<Account>,
    #[serde(default)] resources: Vec<Resource>,
    #[serde(default)] resource_trash: Vec<Resource>,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct PublicAccount<'a> {
    id: &'a str, category_id: &'a str, title: &'a str, username: &'a str,
    website: &'a str, notes: &'a str, icon: &'a str,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct Snapshot<'a> {
    categories: &'a Vec<Category>,
    accounts: Vec<PublicAccount<'a>>,
    settings: &'a VaultSettings,
    resources: &'a Vec<Resource>,
    resource_trash: &'a Vec<Resource>,
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct Status { exists: bool, unlocked: bool }

struct Session {
    dek: Zeroizing<[u8; 32]>, data: VaultData, file: VaultFile, last_activity: Instant,
}
impl Session {
    fn snapshot(&self) -> Snapshot<'_> {
        Snapshot { categories: &self.data.categories,
            accounts: self.data.accounts.iter().map(|a| PublicAccount {
                id: &a.id, category_id: &a.category_id, title: &a.title,
                username: &a.username, website: &a.website, notes: &a.notes, icon: &a.icon,
            }).collect(), settings: &self.data.settings,
            resources: &self.data.resources, resource_trash: &self.data.resource_trash }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        for a in &mut self.data.accounts {
            a.id.zeroize(); a.category_id.zeroize(); a.title.zeroize();
            a.username.zeroize(); a.password.zeroize(); a.website.zeroize();
            a.notes.zeroize(); a.icon.zeroize();
        }
        for a in &mut self.data.trash {
            a.id.zeroize(); a.category_id.zeroize(); a.title.zeroize();
            a.username.zeroize(); a.password.zeroize(); a.website.zeroize();
            a.notes.zeroize(); a.icon.zeroize();
        }
        for c in &mut self.data.categories {
            c.id.zeroize(); c.name.zeroize(); c.icon.zeroize(); c.color.zeroize();
        }
        for r in self.data.resources.iter_mut().chain(self.data.resource_trash.iter_mut()) {
            r.id.zeroize(); r.url.zeroize(); r.title.zeroize(); r.description.zeroize();
            r.notes.zeroize(); r.collection.zeroize(); r.kind.zeroize();
            r.folder_path.zeroize(); r.docs_url.zeroize(); r.updated_at.zeroize(); r.metadata_status.zeroize();
            for tag in &mut r.tags { tag.zeroize(); }
        }
    }
}
struct VaultState(Arc<Mutex<Option<Session>>>);
impl Default for VaultState { fn default() -> Self { Self(Arc::new(Mutex::new(None))) } }

#[cfg(windows)]
mod windows_lock {
    use super::*;
    use std::{cell::RefCell, mem::zeroed, ptr::{null, null_mut}, sync::mpsc, thread};
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{LibraryLoader::GetModuleHandleW, RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION}},
        UI::WindowsAndMessaging::{CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW,
            RegisterClassW, TranslateMessage, MSG, WNDCLASSW, WM_POWERBROADCAST,
            WM_WTSSESSION_CHANGE, WTS_SESSION_LOCK, PBT_APMSUSPEND, PBT_APMRESUMEAUTOMATIC,
            PBT_APMRESUMESUSPEND},
    };
    thread_local! { static STATE: RefCell<Option<Arc<Mutex<Option<Session>>>>> = const { RefCell::new(None) }; }
    unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if (message==WM_WTSSESSION_CHANGE && wparam==WTS_SESSION_LOCK as usize)
            || (message==WM_POWERBROADCAST && [PBT_APMSUSPEND,PBT_APMRESUMEAUTOMATIC,PBT_APMRESUMESUSPEND].contains(&(wparam as u32))) {
            STATE.with(|cell| { if let Some(state)=cell.borrow().as_ref() { if let Ok(mut guard)=state.lock() { *guard=None; } } });
        }
        DefWindowProcW(hwnd,message,wparam,lparam)
    }
    pub fn start(state: Arc<Mutex<Option<Session>>>) -> Result<(),String> {
        let (tx,rx)=mpsc::sync_channel(1);
        thread::Builder::new().name("windows-vault-lock".into()).spawn(move || unsafe {
            STATE.with(|cell| *cell.borrow_mut()=Some(state));
            let class: Vec<u16>="MassKeyVaultLockEvents\0".encode_utf16().collect();
            let instance=GetModuleHandleW(null());
            if instance.is_null() { let _=tx.send(Err("Không thể lấy module Windows".to_string())); return; }
            let descriptor=WNDCLASSW { lpfnWndProc:Some(window_proc), lpszClassName:class.as_ptr(), hInstance:instance, ..zeroed() };
            if RegisterClassW(&descriptor)==0 { let _=tx.send(Err("Không thể đăng ký sự kiện Windows".to_string())); return; }
            // A hidden top-level window receives session and power broadcasts.
            let hwnd=CreateWindowExW(0,class.as_ptr(),class.as_ptr(),0,0,0,0,0,null_mut(),null_mut(),instance,null());
            if hwnd.is_null() { let _=tx.send(Err("Không thể tạo cửa sổ nhận sự kiện Windows".to_string())); return; }
            if WTSRegisterSessionNotification(hwnd,NOTIFY_FOR_THIS_SESSION)==0 {
                let _=tx.send(Err("Không thể theo dõi khóa phiên Windows".to_string())); return;
            }
            let _=tx.send(Ok(()));
            let mut message: MSG=zeroed();
            while GetMessageW(&mut message,null_mut(),0,0)>0 { TranslateMessage(&message); DispatchMessageW(&message); }
        }).map_err(err)?;
        rx.recv_timeout(Duration::from_secs(5)).map_err(|_|"Không nhận được xác nhận theo dõi sự kiện Windows".to_string())?
    }
}

fn err<E: std::fmt::Display>(_: E) -> String { "Không thể thực hiện thao tác. Kiểm tra mật khẩu, định dạng file hoặc quyền truy cập.".into() }
fn storage_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(err)?;
    fs::create_dir_all(&dir).map_err(err)?;
    Ok(dir.join("vault.nvault"))
}
fn rand_bytes<const N: usize>() -> [u8; N] { let mut b = [0; N]; OsRng.fill_bytes(&mut b); b }
fn derive_key(password: &str, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>, String> {
    let params = Params::new(MEMORY_KIB, ITERATIONS, PARALLELISM, Some(32)).map_err(err)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; 32]);
    argon.hash_password_into(password.as_bytes(), salt, &mut *key).map_err(err)?;
    Ok(key)
}
fn seal(key: &[u8; 32], bytes: &[u8], aad: &[u8]) -> Result<BoxedCipher, String> {
    let nonce = rand_bytes::<24>();
    let c = XChaCha20Poly1305::new(Key::from_slice(key));
    let encrypted = c.encrypt(XNonce::from_slice(&nonce), Payload { msg: bytes, aad }).map_err(err)?;
    Ok(BoxedCipher { nonce: B64.encode(nonce), ciphertext: B64.encode(encrypted) })
}
fn open_sealed(key: &[u8; 32], boxed: &BoxedCipher, aad: &[u8]) -> Result<Zeroizing<Vec<u8>>, String> {
    let nonce = B64.decode(&boxed.nonce).map_err(err)?;
    if nonce.len() != 24 { return Err("File mã hóa không hợp lệ".into()); }
    let data = B64.decode(&boxed.ciphertext).map_err(err)?;
    let c = XChaCha20Poly1305::new(Key::from_slice(key));
    Ok(Zeroizing::new(c.decrypt(XNonce::from_slice(&nonce), Payload { msg: &data, aad }).map_err(err)?))
}
fn bytes32(input: &[u8]) -> Result<Zeroizing<[u8; 32]>, String> {
    let key: [u8; 32] = input.try_into().map_err(err)?;
    Ok(Zeroizing::new(key))
}
fn parse_vault(bytes: &[u8]) -> Result<VaultFile, String> {
    if bytes.len() as u64 > MAX_VAULT_BYTES { return Err("File vượt giới hạn 24 MB.".into()); }
    let file: VaultFile = serde_json::from_slice(bytes).map_err(err)?;
    if file.version != VERSION || file.memory_kib != MEMORY_KIB || file.iterations != ITERATIONS || file.parallelism != PARALLELISM {
        return Err("Định dạng vault hoặc tham số bảo mật chưa được hỗ trợ.".into());
    }
    if B64.decode(&file.salt).map_err(err)?.len() != 16 { return Err("Salt không hợp lệ".into()); }
    Ok(file)
}
fn read_vault(path: &Path) -> Result<VaultFile, String> {
    let metadata = fs::metadata(path).map_err(err)?;
    if metadata.len() > MAX_VAULT_BYTES { return Err("File vault quá lớn".into()); }
    parse_vault(&fs::read(path).map_err(err)?)
}
fn unwrap_file(file: &VaultFile, secret: &str, recovery: bool) -> Result<(Zeroizing<[u8; 32]>, VaultData), String> {
    let k: Zeroizing<[u8; 32]>;
    if recovery {
        let compact: String = secret.chars().filter(|c| *c != '-' && !c.is_whitespace()).collect();
        let key = Zeroizing::new(hex::decode(compact).map_err(err)?);
        k = bytes32(&key)?;
    } else {
        let raw = B64.decode(&file.salt).map_err(err)?;
        let salt: [u8; 16] = raw.as_slice().try_into().map_err(err)?;
        k = derive_key(secret, &salt)?;
    }
    let boxed = if recovery { &file.recovery_wrap } else { &file.master_wrap };
    let dek_vec = open_sealed(&k, boxed, if recovery { AAD_RECOVERY } else { AAD_MASTER })?;
    let dek = bytes32(&dek_vec)?;
    let plaintext = open_sealed(&dek, &file.payload, AAD_DATA)?;
    let data: VaultData = serde_json::from_slice(&plaintext).map_err(err)?;
    if !(30..=3600).contains(&data.settings.auto_lock_seconds) || !(5..=120).contains(&data.settings.clipboard_seconds) {
        return Err("Tùy chọn lưu trong vault không hợp lệ.".into());
    }
    Ok((dek, data))
}
fn encrypt_file(dek: &[u8; 32], data: &VaultData, mut file: VaultFile) -> Result<VaultFile, String> {
    let clear = Zeroizing::new(serde_json::to_vec(data).map_err(err)?);
    file.payload = seal(dek, &clear, AAD_DATA)?;
    Ok(file)
}
fn make_file(password: &str, data: &VaultData) -> Result<(VaultFile, Zeroizing<[u8; 32]>, String), String> {
    let salt = rand_bytes::<16>();
    let recovery_key = Zeroizing::new(rand_bytes::<32>());
    let dek = Zeroizing::new(rand_bytes::<32>());
    let master = derive_key(password, &salt)?;
    let file = VaultFile {
        version: VERSION, salt: B64.encode(salt), memory_kib: MEMORY_KIB,
        iterations: ITERATIONS, parallelism: PARALLELISM,
        master_wrap: seal(&master, &dek[..], AAD_MASTER)?,
        recovery_wrap: seal(&recovery_key, &dek[..], AAD_RECOVERY)?,
        payload: seal(&dek, &serde_json::to_vec(data).map_err(err)?, AAD_DATA)?,
    };
    let recovered = hex::encode_upper(&recovery_key[..]);
    let formatted = recovered.as_bytes().chunks(4).map(|v| std::str::from_utf8(v).unwrap_or("")).collect::<Vec<_>>().join("-");
    Ok((file, dek, formatted))
}
fn save_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_VAULT_BYTES { return Err("Vault vượt giới hạn dung lượng".into()); }
    let tmp = path.with_extension(format!("{}.tmp", Uuid::new_v4()));
    let res = (|| -> Result<(), String> {
        let mut out = OpenOptions::new().write(true).create_new(true).open(&tmp).map_err(err)?;
        out.write_all(bytes).map_err(err)?;
        out.sync_all().map_err(err)?;
        drop(out);
        if path.exists() {
            #[cfg(windows)] {
                use std::os::windows::ffi::OsStrExt;
                use windows_sys::Win32::Storage::FileSystem::ReplaceFileW;
                let wide = |v: &Path| -> Vec<u16> { v.as_os_str().encode_wide().chain(std::iter::once(0)).collect() };
                let original = wide(path);
                let replacement = wide(&tmp);
                let backup_path = path.with_extension("bak");
                // Encrypted previous generation is kept to reduce corruption risk.
                if backup_path.exists() { let _ = fs::remove_file(&backup_path); }
                let backup = wide(&backup_path);
                let success = unsafe { ReplaceFileW(original.as_ptr(), replacement.as_ptr(), backup.as_ptr(), 0, std::ptr::null(), std::ptr::null()) };
                if success == 0 { return Err("Không thể thay thế vault an toàn; file gốc vẫn được giữ.".into()); }
            }
            #[cfg(not(windows))] { fs::rename(&tmp, path).map_err(err)?; }
        } else { fs::rename(&tmp, path).map_err(err)?; }
        Ok(())
    })();
    if res.is_err() { let _ = fs::remove_file(&tmp); }
    res
}
fn write_vault(path: &Path, file: &VaultFile) -> Result<(), String> {
    save_atomic(path, &serde_json::to_vec(file).map_err(err)?)
}
fn validated_password(s: &str) -> Result<(), String> {
    if s.chars().count() < 12 { Err("Master Password cần ít nhất 12 ký tự.".into()) } else { Ok(()) }
}
fn with_session<T>(state: &State<'_, VaultState>, f: impl FnOnce(&mut Session) -> Result<T,String>) -> Result<T,String> {
    let mut guard = state.0.lock().map_err(err)?;
    let expired = guard.as_ref().map(|s| s.last_activity.elapsed() >= Duration::from_secs(s.data.settings.auto_lock_seconds)).unwrap_or(false);
    if expired { *guard = None; }
    let session = guard.as_mut().ok_or("Vault đang khóa hoặc đã tự khóa.")?;
    f(session)
}
fn persist_session(app: &AppHandle, session: &mut Session) -> Result<(), String> {
    let next = encrypt_file(&session.dek, &session.data, session.file.clone())?;
    write_vault(&storage_path(app)?, &next)?;
    session.file = next;
    session.last_activity = Instant::now();
    Ok(())
}
fn persist_next_data(app: &AppHandle, session: &mut Session, next: VaultData) -> Result<(), String> {
    persist_next_at(&storage_path(app)?, session, next)
}
fn persist_next_at(path: &Path, session: &mut Session, next: VaultData) -> Result<(), String> {
    let file = encrypt_file(&session.dek, &next, session.file.clone())?;
    write_vault(path, &file)?;
    session.data = next;
    session.file = file;
    session.last_activity = Instant::now();
    Ok(())
}

#[tauri::command]
fn vault_status(app: AppHandle, state: State<'_, VaultState>) -> Result<Status,String> {
    let path = storage_path(&app)?;
    let mut g = state.0.lock().map_err(err)?;
    if g.as_ref().map(|s| s.last_activity.elapsed() >= Duration::from_secs(s.data.settings.auto_lock_seconds)).unwrap_or(false) { *g = None; }
    Ok(Status { exists: path.exists(), unlocked: g.is_some() })
}
#[tauri::command]
fn create_vault(app: AppHandle, state: State<'_, VaultState>, master_password: String) -> Result<String,String> {
    let master = Zeroizing::new(master_password);
    validated_password(&master)?;
    let path = storage_path(&app)?;
    if path.exists() { return Err("Vault đã tồn tại. Không ghi đè dữ liệu.".into()); }
    let data = VaultData::default();
    let (file, dek, recovery) = make_file(&master, &data)?;
    write_vault(&path, &file)?;
    *state.0.lock().map_err(err)? = Some(Session {dek, data, file, last_activity: Instant::now()});
    Ok(recovery)
}
#[tauri::command]
fn unlock_vault(app: AppHandle, state: State<'_, VaultState>, master_password: String) -> Result<(),String> {
    let password = Zeroizing::new(master_password);
    let file = read_vault(&storage_path(&app)?)?;
    let (dek,data) = unwrap_file(&file, &password, false)?;
    *state.0.lock().map_err(err)? = Some(Session {dek,data,file,last_activity:Instant::now()});
    Ok(())
}
#[tauri::command]
fn recover_vault(app: AppHandle, state: State<'_, VaultState>, recovery_key: String, new_master_password: String) -> Result<(),String> {
    let secret = Zeroizing::new(recovery_key);
    let pass = Zeroizing::new(new_master_password);
    validated_password(&pass)?;
    let path = storage_path(&app)?;
    let mut file = read_vault(&path)?;
    let (dek,data) = unwrap_file(&file, &secret, true)?;
    let salt = rand_bytes::<16>();
    let k = derive_key(&pass, &salt)?;
    file.salt = B64.encode(salt);
    file.master_wrap = seal(&k, &dek[..], AAD_MASTER)?;
    write_vault(&path, &file)?;
    *state.0.lock().map_err(err)? = Some(Session {dek,data,file,last_activity:Instant::now()});
    Ok(())
}
#[tauri::command]
fn rotate_recovery(app: AppHandle, state: State<'_, VaultState>, master_password: String) -> Result<String,String> {
    let master = Zeroizing::new(master_password);
    with_session(&state, |s| {
        let (verified, _) = unwrap_file(&s.file, &master, false)?;
        if *verified != *s.dek { return Err("Master Password không hợp lệ".into()); }
        let new_key = Zeroizing::new(rand_bytes::<32>());
        let mut file = s.file.clone();
        file.recovery_wrap = seal(&new_key, &s.dek[..], AAD_RECOVERY)?;
        write_vault(&storage_path(&app)?, &file)?;
        s.file = file;
        s.last_activity = Instant::now();
        let recovered = hex::encode_upper(&new_key[..]);
        Ok(recovered.as_bytes().chunks(4).map(|v| std::str::from_utf8(v).unwrap_or("")).collect::<Vec<_>>().join("-"))
    })
}
#[tauri::command]
fn lock_vault(state: State<'_, VaultState>) -> Result<(),String> {
    *state.0.lock().map_err(err)? = None;
    Ok(())
}
#[tauri::command]
fn touch_session(state: State<'_, VaultState>) -> Result<(),String> {
    with_session(&state, |s| { s.last_activity = Instant::now(); Ok(()) })
}
#[tauri::command]
fn snapshot(state: State<'_, VaultState>) -> Result<serde_json::Value,String> {
    with_session(&state, |s| serde_json::to_value(s.snapshot()).map_err(err))
}
#[tauri::command]
fn get_password(state: State<'_, VaultState>, account_id: String, reauth_password: Option<String>) -> Result<String,String> {
    let secret = Zeroizing::new(reauth_password.unwrap_or_default());
    with_session(&state, |s| {
        if s.data.settings.require_reauth {
            let raw = B64.decode(&s.file.salt).map_err(err)?;
            let salt: [u8; 16] = raw.as_slice().try_into().map_err(err)?;
            let key = derive_key(&secret, &salt)?;
            let verified = bytes32(&open_sealed(&key, &s.file.master_wrap, AAD_MASTER)?)?;
            if *verified != *s.dek { return Err("Xác thực thất bại".into()); }
        }
        s.data.accounts.iter().find(|a| a.id == account_id).map(|a| a.password.clone()).ok_or("Không tìm thấy tài khoản".into())
    })
}
#[tauri::command]
fn upsert_category(app: AppHandle, state: State<'_, VaultState>, id: Option<String>, name: String, icon: String, color: String) -> Result<String,String> {
    if name.trim().is_empty() || name.chars().count()>80 || !valid_icon(&icon) || color.len()>32 { return Err("Thông tin danh mục không hợp lệ".into()); }
    with_session(&state, |s| {
        let category_id = id.unwrap_or_else(|| Uuid::new_v4().to_string());
        if let Some(c) = s.data.categories.iter_mut().find(|c| c.id==category_id) {
            c.name=name; c.icon=icon; c.color=color;
        } else { s.data.categories.push(Category {id:category_id.clone(),name,icon,color}); }
        persist_session(&app,s)?;
        Ok(category_id)
    })
}
// Persist ONLY normalized small PNGs or short built-in icon labels. Image data is part
// of the authenticated-encrypted vault payload and survives ZIP migration.
fn valid_icon(icon: &str) -> bool {
    if let Some(encoded) = icon.strip_prefix("data:image/png;base64,") {
        if encoded.len() > 90000 { return false; }
        let Ok(bytes) = B64.decode(encoded) else { return false; };
        if bytes.len() > 65000 || bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" { return false; }
        let w = u32::from_be_bytes(bytes[16..20].try_into().unwrap_or_default());
        let h = u32::from_be_bytes(bytes[20..24].try_into().unwrap_or_default());
        w > 0 && h > 0 && w <= 128 && h <= 128
    } else {
        // Built-in icons are short Unicode symbols/text, never URIs or markup.
        // In particular, an SVG data URL can fit within 64 bytes, so length
        // and angle-bracket checks alone do not make it safe.
        icon.len() <= 64
            && !icon.to_ascii_lowercase().starts_with("data:")
            && !icon.contains("://")
            && !icon.contains('<')
            && !icon.contains('>')
            && !icon.chars().any(char::is_control)
    }
}

#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct HealthReport { weak: Vec<String>, reused: Vec<String>, total: usize }

// Offline heuristic for audit; UI entry meter applies additional guidance.
fn weak_password(p: &str) -> bool {
    let len = p.chars().count();
    if len < 12 { return true; }
    let mut kinds = 0;
    if p.chars().any(|c| c.is_ascii_lowercase()) { kinds += 1; }
    if p.chars().any(|c| c.is_ascii_uppercase()) { kinds += 1; }
    if p.chars().any(|c| c.is_ascii_digit()) { kinds += 1; }
    if p.chars().any(|c| !c.is_ascii_alphanumeric()) { kinds += 1; }
    let lower = p.to_ascii_lowercase();
    let common = ["password", "qwerty", "123456", "admin", "letmein", "abcdef", "iloveyou"];
    (len < 16 && kinds <= 2) || common.iter().any(|word| lower.contains(word))
        || p.chars().collect::<std::collections::HashSet<_>>().len() <= 3
}
#[tauri::command]
fn vault_health(state: State<'_, VaultState>) -> Result<HealthReport, String> {
    with_session(&state, |s| {
        let mut seen = std::collections::HashMap::<&str, Vec<String>>::new();
        let mut weak = Vec::new();
        for a in &s.data.accounts {
            if !a.password.is_empty() {
                if weak_password(&a.password) { weak.push(a.id.clone()); }
                seen.entry(&a.password).or_default().push(a.id.clone());
            }
        }
        let reused = seen.values().filter(|ids| ids.len()>1).flat_map(|ids| ids.iter().cloned()).collect();
        Ok(HealthReport { weak, reused, total:s.data.accounts.len() })
    })
}
#[tauri::command]
fn open_account_website(app: AppHandle, state: State<'_, VaultState>, account_id: String) -> Result<(), String> {
    let website = with_session(&state, |s| {
        s.data.accounts.iter().find(|a| a.id == account_id).map(|a| a.website.clone()).ok_or("Không tìm thấy tài khoản".into())
    })?;
    if website.len() > 2048 || website.chars().any(|c| c.is_whitespace() || c.is_control() || c == '\\') {
        return Err("Địa chỉ website không hợp lệ".into());
    }
    let url = url::Url::parse(&website).map_err(|_| "Cần website https:// hoặc http://".to_string())?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err("Chỉ hỗ trợ liên kết HTTP/HTTPS không có thông tin đăng nhập trong URL".into());
    }
    app.opener().open_url(website, None::<&str>).map_err(err)
}
#[tauri::command]
fn generate_similar_password(original: String) -> Result<String, String> {
    // A short familiar hint plus at least 22 unpredictable random symbols;
    // NEVER deterministic or a simple replacement/substitution of the old password.
    let hint: String = original.chars().filter(|c| c.is_ascii_alphabetic()).take(3).collect();
    let suffix = generate_password(24)?;
    Ok(format!("{}{}", hint, suffix))
}

#[tauri::command]
fn delete_category(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |s| {
        let mut next = s.data.clone();
        remove_empty_category(&mut next, &id)?;
        persist_next_data(&app,s,next)
    })
}
fn remove_empty_category(data: &mut VaultData, id: &str) -> Result<(),String> {
    if data.accounts.iter().chain(data.trash.iter()).any(|a| a.category_id == id) { return Err("Chuyển hoặc xóa vĩnh viễn tài khoản liên quan trước khi xóa danh mục.".into()); }
    if !data.categories.iter().any(|c| c.id == id) { return Err("Không tìm thấy danh mục.".into()); }
    data.categories.retain(|c| c.id != id);
    Ok(())
}
#[derive(Deserialize)]
#[serde(rename_all="camelCase", deny_unknown_fields)]
struct AccountInput { id: Option<String>, category_id: String, title: String, username: String, password: Option<String>, website: String, notes: String, icon: String }
#[tauri::command]
fn upsert_account(app: AppHandle, state: State<'_, VaultState>, account: AccountInput) -> Result<String,String> {
    if account.title.trim().is_empty() || account.title.len()>300 || account.username.len()>1024 || account.website.len()>2048 || account.notes.len()>16384 || !valid_icon(&account.icon) || account.password.as_ref().map(|s| s.len()>16384).unwrap_or(false) {
        return Err("Trường dữ liệu không hợp lệ hoặc quá dài".into());
    }
    with_session(&state, |s| {
        if !account.category_id.is_empty() && !s.data.categories.iter().any(|c|c.id==account.category_id) {return Err("Danh mục không tồn tại.".into());}
        let id = account.id.clone().unwrap_or_else(||Uuid::new_v4().to_string());
        if let Some(existing) = s.data.accounts.iter_mut().find(|a|a.id==id) {
            existing.category_id=account.category_id; existing.title=account.title;
            existing.username=account.username; existing.website=account.website;
            existing.notes=account.notes; existing.icon=account.icon;
            if let Some(p)=account.password { existing.password.zeroize(); existing.password=p; }
        } else {
            s.data.accounts.push(Account { id:id.clone(),category_id:account.category_id,title:account.title,
                username:account.username,password:account.password.unwrap_or_default(),website:account.website,
                notes:account.notes,icon:account.icon });
        }
        persist_session(&app,s)?;
        Ok(id)
    })
}
#[tauri::command]
fn delete_account(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |s| {
        if let Some(pos)=s.data.accounts.iter().position(|a|a.id==id) {
            let mut next=s.data.clone();
            next.trash.push(next.accounts.remove(pos));
            let file=encrypt_file(&s.dek,&next,s.file.clone())?;
            write_vault(&storage_path(&app)?,&file)?;
            s.data=next; s.file=file; s.last_activity=Instant::now();
        }
        Ok(())
    })
}
#[tauri::command]
fn restore_account(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |s| {
        let pos=s.data.trash.iter().position(|a|a.id==id).ok_or("Không tìm thấy tài khoản trong thùng rác")?;
        let mut next=s.data.clone();
        let mut account=next.trash.remove(pos);
        if next.accounts.iter().any(|a|a.id==account.id) { account.id=Uuid::new_v4().to_string(); }
        next.accounts.push(account);
        let file=encrypt_file(&s.dek,&next,s.file.clone())?;
        write_vault(&storage_path(&app)?,&file)?;
        s.data=next; s.file=file; s.last_activity=Instant::now();
        Ok(())
    })
}
#[tauri::command]
fn trash_snapshot(state: State<'_, VaultState>) -> Result<serde_json::Value,String> {
    with_session(&state, |s| serde_json::to_value(s.data.trash.iter().map(|a| PublicAccount {
        id:&a.id,category_id:&a.category_id,title:&a.title,username:&a.username,
        website:&a.website,notes:&a.notes,icon:&a.icon,
    }).collect::<Vec<_>>()).map_err(err))
}
#[tauri::command]
fn permanently_delete_account(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |s| {
        let mut next = s.data.clone();
        remove_trashed_account(&mut next, &id)?;
        persist_next_data(&app, s, next)
    })
}
fn remove_trashed_account(data: &mut VaultData, id: &str) -> Result<(),String> {
    let pos = data.trash.iter().position(|a| a.id == id).ok_or("Không tìm thấy tài khoản trong thùng rác")?;
    data.trash.remove(pos);
    Ok(())
}
#[tauri::command]
fn empty_trash(app: AppHandle, state: State<'_, VaultState>) -> Result<(),String> {
    with_session(&state, |s| {
        if s.data.trash.is_empty() { return Ok(()); }
        let mut next = s.data.clone();
        next.trash.clear();
        persist_next_data(&app, s, next)
    })
}
#[tauri::command]
fn update_settings(app: AppHandle, state: State<'_, VaultState>, auto_lock_seconds: u64, clipboard_seconds: u64, lock_on_minimize: bool, require_reauth: bool, warn_weak_passwords: bool, vault_health: bool) -> Result<(),String> {
    if !(30..=3600).contains(&auto_lock_seconds) || !(5..=120).contains(&clipboard_seconds) { return Err("Thời gian phải nằm trong giới hạn cho phép.".into()); }
    with_session(&state, |s| {
        s.data.settings=VaultSettings {auto_lock_seconds, clipboard_seconds,lock_on_minimize,require_reauth,warn_weak_passwords,vault_health};
        persist_session(&app,s)
    })
}
#[tauri::command]
fn change_master(app: AppHandle, state: State<'_, VaultState>, current_password: String, new_password: String) -> Result<(),String> {
    let old = Zeroizing::new(current_password);
    let new = Zeroizing::new(new_password);
    validated_password(&new)?;
    with_session(&state, |s| {
        let (validated,_) = unwrap_file(&s.file, &old, false)?;
        if *validated != *s.dek { return Err("Master Password hiện tại không đúng".into()); }
        let salt=rand_bytes::<16>();
        let k=derive_key(&new,&salt)?;
        let mut file=s.file.clone(); file.salt=B64.encode(salt);
        file.master_wrap=seal(&k,&s.dek[..],AAD_MASTER)?;
        write_vault(&storage_path(&app)?,&file)?;
        s.file=file; s.last_activity=Instant::now();
        Ok(())
    })
}
#[tauri::command]
fn generate_password(length: usize) -> Result<String,String> {
    if !(12..=128).contains(&length) {return Err("Độ dài mật khẩu phải từ 12 đến 128".into());}
    const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789!@#$%^&*_-+=";
    let mut rng=OsRng;
    Ok((0..length).map(|_| ALPHABET[rng.gen_range(0..ALPHABET.len())] as char).collect())
}
fn checked_web_url(raw: &str) -> Result<url::Url,String> {
    if raw.len()>2048 || raw.chars().any(|c| c.is_control() || c.is_whitespace() || c=='\\') {
        return Err("URL quá dài hoặc chứa ký tự không hợp lệ".into());
    }
    let url=url::Url::parse(raw).map_err(|_|"URL cần bắt đầu bằng https:// hoặc http://".to_string())?;
    if !matches!(url.scheme(),"https"|"http") || url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
        return Err("Chỉ hỗ trợ URL HTTP/HTTPS không chứa thông tin đăng nhập".into());
    }
    Ok(url)
}
fn valid_resource(r: &Resource) -> Result<(),String> {
    checked_web_url(&r.url)?;
    if !r.docs_url.is_empty() { checked_web_url(&r.docs_url)?; }
    if !matches!(r.kind.as_str(),"website"|"github"|"document") || r.title.trim().is_empty() || r.title.len()>240 ||
        r.description.len()>600 || r.notes.len()>20_000 || r.collection.len()>120 || r.folder_path.len()>4096 ||
        r.tags.len()>24 || r.tags.iter().any(|tag|tag.len()>64) { return Err("Thông tin tài nguyên không hợp lệ".into()); }
    if !r.folder_path.is_empty() && (!Path::new(&r.folder_path).is_absolute() || r.folder_path.chars().any(|c|c.is_control())) {
        return Err("Đường dẫn thư mục phải là đường dẫn tuyệt đối".into());
    }
    Ok(())
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct ResourceMetadata { title: String, description: String, stars: Option<u64>, updated_at: String, docs_url: String, status: String }
fn public_address(ip: std::net::IpAddr) -> bool {
    match ip {
        std::net::IpAddr::V4(v) => !v.is_private() && !v.is_loopback() && !v.is_link_local() && !v.is_broadcast() && !v.is_documentation() && !v.is_unspecified() && v.octets()[0]!=0 && v.octets()[0]<224 && v.octets()[0]!=100,
        std::net::IpAddr::V6(v) => v.to_ipv4_mapped().map(|mapped|public_address(std::net::IpAddr::V4(mapped)))
            .unwrap_or_else(||!v.is_loopback() && !v.is_unspecified() && !v.is_unique_local() && !v.is_unicast_link_local() && !v.is_multicast()),
    }
}
fn metadata_response(raw: &str, accept: &str) -> Result<(String,reqwest::header::HeaderMap),String> {
    use std::net::ToSocketAddrs;
    let url=checked_web_url(raw)?;
    if url.scheme()!="https" {return Err("Chỉ lấy metadata qua HTTPS".into());}
    let host=url.host_str().ok_or("URL thiếu tên miền")?;
    if host.parse::<std::net::IpAddr>().is_ok() || host.eq_ignore_ascii_case("localhost") || !host.contains('.') {return Err("Không lấy metadata từ địa chỉ nội bộ".into());}
    let addresses=(host,443).to_socket_addrs().map_err(|_|"Không phân giải được tên miền".to_string())?.collect::<Vec<_>>();
    if addresses.is_empty() || addresses.iter().any(|addr|!public_address(addr.ip())) {return Err("Tên miền trỏ đến mạng nội bộ".into());}
    let client=reqwest::blocking::Client::builder().timeout(Duration::from_secs(5)).no_proxy()
        .redirect(reqwest::redirect::Policy::none()).resolve(host,addresses[0]).build().map_err(err)?;
    let response=client.get(url).header(reqwest::header::USER_AGENT,"MassKey-Vault/1.3 metadata")
        .header(reqwest::header::ACCEPT,accept).send().map_err(|_|"Không thể tải metadata".to_string())?;
    if !response.status().is_success() {return Err(format!("Metadata HTTP {}",response.status().as_u16()));}
    if response.content_length().unwrap_or(0)>128*1024 {return Err("Metadata vượt giới hạn 128 KB".into());}
    let headers=response.headers().clone();
    let mut body=Vec::new();
    response.take(128*1024+1).read_to_end(&mut body).map_err(err)?;
    if body.len()>128*1024 {return Err("Metadata vượt giới hạn 128 KB".into());}
    Ok((String::from_utf8_lossy(&body).into_owned(),headers))
}
fn html_field(html: &str, name: &str) -> String {
    let lower=html.to_ascii_lowercase();
    if name=="title" {
        if let Some(start)=lower.find("<title") { if let Some(open)=lower[start..].find('>') { let from=start+open+1;
            if let Some(end)=lower[from..].find("</title>") {return html[from..from+end].trim().chars().take(240).collect();}
        } } }
    for (start,_) in lower.match_indices("<meta") {
        if let Some(end)=lower[start..].find('>') {let tag=&html[start..start+end];let low=tag.to_ascii_lowercase();
            if low.contains(&format!("name=\"{}\"",name)) || low.contains(&format!("property=\"og:{}\"",name)) {
                if let Some(pos)=low.find("content=\"") {let from=pos+9;if let Some(stop)=tag[from..].find('"') {return tag[from..from+stop].trim().chars().take(600).collect();}}
            }
        }
    }
    String::new()
}
fn github_repo(url: &url::Url) -> Option<(String,String)> {
    if url.host_str()? != "github.com" {return None;}
    let mut parts=url.path_segments()?;
    let owner=parts.next()?.to_string();let repo=parts.next()?.trim_end_matches(".git").to_string();
    if owner.is_empty() || repo.is_empty() || !owner.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-') ||
       !repo.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-'||b==b'_'||b==b'.') {return None;}
    Some((owner,repo))
}
fn discover_github_docs(owner:&str,repo:&str,branch:&str,exists:impl Fn(&str)->bool) -> String {
    let base=format!("https://github.com/{owner}/{repo}/tree/{branch}");
    for dir in ["docs","doc","documentation"] {
        let endpoint=format!("https://api.github.com/repos/{owner}/{repo}/contents/{dir}");
        if exists(&endpoint) {return format!("{base}/{dir}");}
    }
    let readme=format!("https://api.github.com/repos/{owner}/{repo}/readme");
    if exists(&readme) {format!("https://github.com/{owner}/{repo}#readme")} else {String::new()}
}
fn fetch_metadata(raw: &str, discover_docs: bool) -> Result<ResourceMetadata,String> {
    let url=checked_web_url(raw)?;
    if let Some((owner,repo))=github_repo(&url) {
        let api=format!("https://api.github.com/repos/{owner}/{repo}");
        let (body,_)=metadata_response(&api,"application/vnd.github+json")?;
        let data:serde_json::Value=serde_json::from_str(&body).map_err(err)?;
        let branch=data["default_branch"].as_str().unwrap_or("main");
        let docs_url=if discover_docs {discover_github_docs(&owner,&repo,branch,|endpoint|
            metadata_response(endpoint,"application/vnd.github+json").is_ok())} else {String::new()};
        return Ok(ResourceMetadata {title:data["full_name"].as_str().unwrap_or(&repo).to_string(),
            description:data["description"].as_str().unwrap_or("").to_string(),
            stars:data["stargazers_count"].as_u64(),updated_at:data["updated_at"].as_str().unwrap_or("").to_string(),
            docs_url,status:"ok".into()});
    }
    let (body,headers)=metadata_response(raw,"text/html")?;
    if !headers.get(reqwest::header::CONTENT_TYPE).and_then(|v|v.to_str().ok()).unwrap_or("").starts_with("text/html") {
        return Err("Trang không trả về HTML; hãy nhập thông tin thủ công".into());
    }
    Ok(ResourceMetadata {title:html_field(&body,"title"),description:html_field(&body,"description"),stars:None,
        updated_at:String::new(),docs_url:String::new(),status:"ok".into()})
}
#[tauri::command]
async fn fetch_resource_metadata(url: String, discover_docs: Option<bool>) -> Result<ResourceMetadata,String> {
    tauri::async_runtime::spawn_blocking(move || fetch_metadata(&url,discover_docs.unwrap_or(true))).await.map_err(err)?
}
#[tauri::command]
fn upsert_resource(app: AppHandle, state: State<'_, VaultState>, mut resource: Resource) -> Result<String,String> {
    valid_resource(&resource)?;
    with_session(&state, |session| {
        let mut next=session.data.clone();
        if resource.id.is_empty() { resource.id=Uuid::new_v4().to_string(); }
        else if !next.resources.iter().any(|r|r.id==resource.id) { return Err("Không tìm thấy tài nguyên để sửa".into()); }
        if let Some(existing)=next.resources.iter_mut().find(|r|r.id==resource.id) { *existing=resource.clone(); }
        else {next.resources.push(resource.clone());}
        persist_next_data(&app,session,next)?;
        Ok(resource.id)
    })
}
#[tauri::command]
fn delete_resource(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |session| {
        let mut next=session.data.clone();
        let index=next.resources.iter().position(|r|r.id==id).ok_or("Không tìm thấy tài nguyên")?;
        next.resource_trash.push(next.resources.remove(index));
        persist_next_data(&app,session,next)
    })
}
#[tauri::command]
fn restore_resource(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |session| {
        let mut next=session.data.clone();
        let index=next.resource_trash.iter().position(|r|r.id==id).ok_or("Không tìm thấy tài nguyên")?;
        next.resources.push(next.resource_trash.remove(index));
        persist_next_data(&app,session,next)
    })
}
#[tauri::command]
fn permanently_delete_resource(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    with_session(&state, |session| {
        let mut next=session.data.clone();
        let index=next.resource_trash.iter().position(|r|r.id==id).ok_or("Không tìm thấy tài nguyên")?;
        next.resource_trash.remove(index);
        persist_next_data(&app,session,next)
    })
}
#[tauri::command]
fn open_resource_url(app: AppHandle, state: State<'_, VaultState>, id: String, docs: bool) -> Result<(),String> {
    let raw=with_session(&state, |session| session.data.resources.iter().find(|r|r.id==id)
        .map(|r| if docs {r.docs_url.clone()} else {r.url.clone()}).ok_or("Không tìm thấy tài nguyên".into()))?;
    let url=checked_web_url(&raw)?;
    app.opener().open_url(url.as_str(),None::<&str>).map_err(err)
}
#[tauri::command]
fn open_resource_folder(app: AppHandle, state: State<'_, VaultState>, id: String) -> Result<(),String> {
    let raw=with_session(&state, |session| session.data.resources.iter().find(|r|r.id==id)
        .map(|r|r.folder_path.clone()).ok_or("Không tìm thấy tài nguyên".into()))?;
    let path=PathBuf::from(raw);
    if !path.is_absolute() || !path.is_dir() { return Err("Thư mục không còn tồn tại. Hãy sửa đường dẫn đã lưu.".into()); }
    app.opener().open_path(path.to_string_lossy().into_owned(),None::<&str>).map_err(err)
}
#[tauri::command]
fn export_zip(app: AppHandle, state: State<'_, VaultState>, destination: String, zip_password: String) -> Result<(),String> {
    use zip::{write::SimpleFileOptions, AesMode, CompressionMethod, ZipWriter};
    if zip_password.chars().count()<16 {return Err("Mật khẩu ZIP cần ít nhất 16 ký tự.".into());}
    // Lock status checked; ZIP is a SECOND layer. Inner blob stays encrypted.
    with_session(&state, |_| Ok(()))?;
    let origin=storage_path(&app)?;
    let dest=PathBuf::from(destination);
    if dest.extension().and_then(|e|e.to_str()).map(|e|!e.eq_ignore_ascii_case("zip")).unwrap_or(true) {return Err("Chỉ xuất file .zip".into());}
    let bytes=fs::read(&origin).map_err(err)?;
    if bytes.len() as u64>MAX_VAULT_BYTES {return Err("Vault vượt giới hạn".into());}
    let _=parse_vault(&bytes)?;
    if dest.exists() {return Err("File ZIP đã tồn tại, hãy chọn tên khác để không ghi đè.".into());}
    let temp=dest.with_extension(format!("{}.tmp",Uuid::new_v4()));
    let pw=Zeroizing::new(zip_password);
    let result=(||->Result<(),String>{
        let out=File::create(&temp).map_err(err)?;
        let mut writer=ZipWriter::new(out);
        let options=SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
            .with_aes_encryption(AesMode::Aes256,&pw);
        writer.start_file("vault.nvault", options).map_err(err)?;
        writer.write_all(&bytes).map_err(err)?;
        let out=writer.finish().map_err(err)?; out.sync_all().map_err(err)?;
        fs::rename(&temp,&dest).map_err(err)?;
        Ok(())
    })();
    if result.is_err() {let _=fs::remove_file(&temp);}
    result
}
fn read_zip(path: &Path, zip_password: &str) -> Result<VaultFile,String> {
    let meta=fs::metadata(path).map_err(err)?;
    if meta.len()>MAX_VAULT_BYTES+1024*1024 {return Err("ZIP quá lớn.".into());}
    let reader=File::open(path).map_err(err)?;
    let mut archive=zip::ZipArchive::new(reader).map_err(err)?;
    if archive.len()!=1 {return Err("Chỉ chấp nhận bản sao MassKey có một file mã hóa.".into());}
    // Reject legacy ZipCrypto and AES-128/192 archives; only AES-256 ZIP backups.
    let aes = archive.get_aes_verification_key_and_salt(0).map_err(err)?
        .ok_or("File ZIP không sử dụng AES.")?;
    if aes.aes_mode != zip::AesMode::Aes256 {
        return Err("ZIP cần sử dụng AES-256".into());
    }
    let mut file=archive.by_index_decrypt(0,zip_password.as_bytes()).map_err(err)?;
    if !file.encrypted() || file.name().map_err(err)?.as_ref()!="vault.nvault" || file.size()>MAX_VAULT_BYTES {
        return Err("ZIP không đúng định dạng backup bảo mật.".into());
    }
    let mut bytes=Zeroizing::new(Vec::new());
    file.by_ref().take(MAX_VAULT_BYTES+1).read_to_end(&mut bytes).map_err(err)?;
    parse_vault(&bytes)
}
#[derive(Serialize)]
#[serde(rename_all="camelCase")]
struct ImportPreview { added: usize, unchanged: usize, conflicts_as_new: usize, replaced: usize, trash_added: usize,
    resources_added: usize, resources_unchanged: usize, resources_conflicts_as_new: usize, resources_replaced: usize, resources_trash_added: usize,
    invalid: usize, fingerprint: String }
fn import_fingerprint(source: &VaultFile, target: Option<&VaultFile>, mode: &str) -> Result<String,String> {
    let mut hash=Sha256::new();
    hash.update(serde_json::to_vec(source).map_err(err)?);
    if let Some(file)=target { hash.update(serde_json::to_vec(file).map_err(err)?); }
    hash.update(mode.as_bytes());
    Ok(hex::encode(hash.finalize()))
}
fn validate_import(data: &VaultData) -> Result<(),String> {
    let mut ids=std::collections::HashSet::new();
    let mut categories=std::collections::HashSet::new();
    for c in &data.categories { if !categories.insert(&c.id) {return Err("Bản sao có mã danh mục trùng".into());} }
    for a in data.accounts.iter().chain(data.trash.iter()) {
        if !ids.insert(&a.id) || (!a.category_id.is_empty() && !categories.contains(&a.category_id)) {
            return Err("Bản sao có tài khoản trùng mã hoặc thiếu danh mục".into());
        }
    }
    let mut resource_ids=std::collections::HashSet::new();
    for resource in data.resources.iter().chain(data.resource_trash.iter()) {
        if !resource_ids.insert(&resource.id) {return Err("Bản sao có tài nguyên trùng mã".into());}
        valid_resource(resource)?;
    }
    Ok(())
}
fn preview_counts(source: &VaultData, target: Option<&VaultData>, mode: &str, fingerprint: String) -> ImportPreview {
    if mode=="replace" { return ImportPreview {added:source.accounts.len(),unchanged:0,conflicts_as_new:0,replaced:target.map(|d|d.accounts.len()).unwrap_or(0),trash_added:source.trash.len(),
        resources_added:source.resources.len(),resources_unchanged:0,resources_conflicts_as_new:0,resources_replaced:target.map(|d|d.resources.len()).unwrap_or(0),resources_trash_added:source.resource_trash.len(),invalid:0,fingerprint}; }
    let mut p=ImportPreview {added:0,unchanged:0,conflicts_as_new:0,replaced:0,trash_added:source.trash.len(),
        resources_added:0,resources_unchanged:0,resources_conflicts_as_new:0,resources_replaced:0,resources_trash_added:source.resource_trash.len(),invalid:0,fingerprint};
    for a in &source.accounts {
        let category_remapped=source.categories.iter().find(|c|c.id==a.category_id)
            .and_then(|c|target.and_then(|d|d.categories.iter().find(|t|t.id==c.id).map(|t|t.name!=c.name || t.icon!=c.icon || t.color!=c.color)))
            .unwrap_or(false);
        match target.and_then(|d|d.accounts.iter().find(|b|b.id==a.id)) {
            None=>p.added+=1,
            Some(b) if !category_remapped && b.category_id==a.category_id && b.title==a.title && b.username==a.username && b.password==a.password && b.website==a.website && b.notes==a.notes && b.icon==a.icon=>p.unchanged+=1,
            Some(_)=>p.conflicts_as_new+=1,
        }
    }
    for resource in &source.resources {
        match target.and_then(|d|d.resources.iter().find(|item|item.id==resource.id)) {
            None=>p.resources_added+=1,
            Some(existing) if existing==resource=>p.resources_unchanged+=1,
            Some(_)=>p.resources_conflicts_as_new+=1,
        }
    }
    p
}
#[tauri::command]
fn preview_import_zip(app: AppHandle, state: State<'_,VaultState>, source: String, zip_password: String,
    source_secret: String, recovery_mode: bool, mode: String) -> Result<ImportPreview,String> {
    if mode!="merge" && mode!="replace" { return Err("Chế độ nhập không hợp lệ".into()); }
    let file=read_zip(&PathBuf::from(source),&Zeroizing::new(zip_password))?;
    let (_,imported)=unwrap_file(&file,&Zeroizing::new(source_secret),recovery_mode)?;
    validate_import(&imported)?;
    let path=storage_path(&app)?;
    let target_file=if path.exists() {Some(read_vault(&path)?)} else {None};
    let target=state.0.lock().map_err(err)?;
    if path.exists() && target.is_none() {return Err("Mở khóa vault hiện tại trước khi xem trước bản nhập".into());}
    let fingerprint=import_fingerprint(&file,target_file.as_ref(),&mode)?;
    Ok(preview_counts(&imported,target.as_ref().map(|s|&s.data),&mode,fingerprint))
}
#[tauri::command]
fn import_zip(app: AppHandle, state: State<'_,VaultState>, source: String, zip_password: String,
    source_secret: String, recovery_mode: bool, new_master_password: Option<String>, mode: String, expected_fingerprint: String) -> Result<usize,String> {
    let zp=Zeroizing::new(zip_password);
    let source_secret=Zeroizing::new(source_secret);
    let file=read_zip(&PathBuf::from(source),&zp)?;
    let (source_dek, mut imported)=unwrap_file(&file,&source_secret,recovery_mode)?;
    validate_import(&imported)?;
    let path=storage_path(&app)?;
    let mut guard=state.0.lock().map_err(err)?;
    let target_file=if path.exists() {Some(read_vault(&path)?)} else {None};
    if import_fingerprint(&file,target_file.as_ref(),&mode)? != expected_fingerprint {return Err("Bản sao hoặc vault đã thay đổi. Hãy xem trước lại.".into());}
    let is_unlocked=guard.as_ref().map(|s|s.last_activity.elapsed() < Duration::from_secs(s.data.settings.auto_lock_seconds)).unwrap_or(false);
    if !is_unlocked { *guard=None; }
    let count=imported.accounts.len();
    if path.exists() && mode=="merge" {
        let target=guard.as_mut().ok_or("Phải mở khóa vault hiện tại để gộp dữ liệu.")?;
        let mut next=target.data.clone();
        // Avoid UUID collisions that could associate imported accounts with
        // the wrong category or silently lose a distinct imported account.
        let mut remap=std::collections::HashMap::<String,String>::new();
        for mut cat in imported.categories.drain(..) {
            if let Some(existing)=next.categories.iter().find(|c|c.id==cat.id) {
                if existing.name!=cat.name || existing.icon!=cat.icon || existing.color!=cat.color {
                    let previous=cat.id.clone(); cat.id=Uuid::new_v4().to_string();
                    remap.insert(previous,cat.id.clone());next.categories.push(cat);
                }
            } else {next.categories.push(cat);}
        }
        for mut acc in imported.accounts.drain(..) {
            if let Some(new_id)=remap.get(&acc.category_id) {acc.category_id=new_id.clone();}
            if let Some(existing)=next.accounts.iter().find(|a|a.id==acc.id) {
                if existing.title!=acc.title || existing.username!=acc.username ||
                   existing.password!=acc.password || existing.website!=acc.website ||
                    existing.notes!=acc.notes || existing.category_id!=acc.category_id || existing.icon!=acc.icon {
                    acc.id=Uuid::new_v4().to_string(); next.accounts.push(acc);
                }
            } else {next.accounts.push(acc);}
        }
        for mut acc in imported.trash.drain(..) {
            if let Some(new_id)=remap.get(&acc.category_id) {acc.category_id=new_id.clone();}
            if next.accounts.iter().chain(next.trash.iter()).any(|a|a.id==acc.id) {
                acc.id=Uuid::new_v4().to_string();
            }
            next.trash.push(acc);
        }
        for mut resource in imported.resources.drain(..) {
            if let Some(existing)=next.resources.iter().find(|r|r.id==resource.id) {
                if existing!=&resource {resource.id=Uuid::new_v4().to_string();next.resources.push(resource);}
            } else if next.resource_trash.iter().any(|r|r.id==resource.id) {
                resource.id=Uuid::new_v4().to_string();next.resources.push(resource);
            } else {next.resources.push(resource);}
        }
        for mut resource in imported.resource_trash.drain(..) {
            if next.resources.iter().chain(next.resource_trash.iter()).any(|r|r.id==resource.id) {
                resource.id=Uuid::new_v4().to_string();
            }
            next.resource_trash.push(resource);
        }
        let file=encrypt_file(&target.dek,&next,target.file.clone())?;
        write_vault(&path,&file)?;
        target.data=next; target.file=file; target.last_activity=Instant::now();
        return Ok(count);
    }
    if mode!="replace" && mode!="merge" {return Err("Chế độ khôi phục không hợp lệ".into());}
    if path.exists() && guard.is_none() {return Err("Phải mở khóa vault hiện tại trước khi thay thế.".to_string());}
    let mut destination=file;
    if recovery_mode {
        let new=Zeroizing::new(new_master_password.ok_or("Cần Master Password mới khi khôi phục bằng Recovery Key.")?);
        validated_password(&new)?;
        let salt=rand_bytes::<16>();
        let k=derive_key(&new,&salt)?;
        destination.salt=B64.encode(salt);
        destination.master_wrap=seal(&k,&source_dek[..],AAD_MASTER)?;
    }
    write_vault(&path,&destination)?;
    *guard=Some(Session{dek:source_dek,data:imported,file:destination,last_activity:Instant::now()});
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "Generates a disposable UAT vault only when explicitly requested"]
    fn write_ui_qa_fixture() {
        let path = std::env::var_os("MASSKEY_UAT_FIXTURE").expect("Set MASSKEY_UAT_FIXTURE to a workspace path");
        let path = PathBuf::from(path);
        let category = |id: &str| Category { id:id.into(), name:id.into(), icon:"✦".into(), color:"#9978fc".into() };
        let account = |id: &str, category_id: &str| Account { id:id.into(), category_id:category_id.into(), title:id.into(), username:"uat@example.invalid".into(), password:"fixture-only-secret".into(), website:String::new(), notes:String::new(), icon:"✦".into() };
        let data = VaultData { categories:vec![category("Empty"),category("Active"),category("Trashed")], accounts:vec![account("Active sample","Active")], trash:vec![account("Trash sample 1","Trashed"),account("Trash sample 2","Trashed")], ..VaultData::default() };
        let (file,_,_) = make_file("MassKey-UAT-Only-2026!", &data).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        write_vault(&path,&file).unwrap();
    }
    #[test]
    fn category_deletion_waits_for_active_and_trashed_accounts() {
        let category=Category{id:"c".into(),name:"Test".into(),icon:"✦".into(),color:"#9978fc".into()};
        let account=Account{id:"a".into(),category_id:"c".into(),title:"Test".into(),username:String::new(),password:"secret".into(),website:String::new(),notes:String::new(),icon:"✦".into()};
        let mut data=VaultData{categories:vec![category],accounts:vec![account.clone()],..VaultData::default()};
        assert!(remove_empty_category(&mut data,"c").is_err());
        assert_eq!(data.categories.len(),1);
        data.trash.push(data.accounts.remove(0));
        assert!(remove_empty_category(&mut data,"c").is_err());
        assert_eq!(data.categories.len(),1);
        assert!(remove_trashed_account(&mut data,"missing").is_err());
        assert_eq!(data.trash.len(),1);
        remove_trashed_account(&mut data,"a").unwrap();
        assert!(data.trash.is_empty());
        remove_empty_category(&mut data,"c").unwrap();
        assert!(data.categories.is_empty());
    }
    #[test]
    fn failed_permanent_delete_write_keeps_open_session() {
        let mut data=VaultData::default();
        data.trash.push(Account{id:"test".into(),category_id:String::new(),title:"Test".into(),username:String::new(),password:"fixture-only".into(),website:String::new(),notes:String::new(),icon:"✦".into()});
        let (file,dek,_)=make_file("MassKey-UAT-Only-2026!",&data).unwrap();
        let mut session=Session{dek,data,file,last_activity:Instant::now()};
        let mut next=session.data.clone();
        remove_trashed_account(&mut next,"test").unwrap();
        let missing=std::env::temp_dir().join(format!("masskey-missing-{}",Uuid::new_v4())).join("vault.nvault");
        assert!(persist_next_at(&missing,&mut session,next).is_err());
        assert_eq!(session.data.trash.len(),1);
        assert_eq!(unwrap_file(&session.file,"MassKey-UAT-Only-2026!",false).unwrap().1.trash.len(),1);
    }
    #[test]
    fn correct_password_roundtrip_and_tamper_failure() {
        let data=VaultData::default();
        let (mut vault,_,recovery)=make_file("a-very-long-test-master-password",&data).unwrap();
        assert!(unwrap_file(&vault,"wrong password",false).is_err());
        assert!(unwrap_file(&vault,"a-very-long-test-master-password",false).is_ok());
        assert!(unwrap_file(&vault,&recovery,true).is_ok());
        vault.payload.ciphertext.push('A');
        assert!(unwrap_file(&vault,"a-very-long-test-master-password",false).is_err());
    }
    #[test]
    fn legacy_settings_upgrade_and_icon_limits() {
        let settings: VaultSettings = serde_json::from_str(r#"{"auto_lock_seconds":180,"clipboard_seconds":25}"#).unwrap();
        assert!(settings.warn_weak_passwords && settings.vault_health);
        assert!(!settings.require_reauth && !settings.lock_on_minimize);
        assert!(valid_icon("🎮"));
        assert!(!valid_icon("data:image/svg+xml;base64,AAAA"));
        assert!(!valid_icon("DATA:image/svg+xml;base64,AAAA"));
        assert!(!valid_icon("data:text/html,hello"));
        assert!(!valid_icon("https://example.com/icon.svg"));
        assert!(!valid_icon("<svg/>"));
        assert!(!valid_icon("hello\nworld"));
        assert!(!valid_icon(&"X".repeat(65)));
        assert!(validated_password("abcdefghijkl").is_ok());
        assert!(validated_password("abcdefghijk").is_err());
        assert!(weak_password("password123"));
    }
    #[test]
    fn strict_data_version_and_settings() {
        let (mut vault,_,_)=make_file("a-very-long-test-master-password",&VaultData::default()).unwrap();
        vault.version=99;
        assert!(parse_vault(&serde_json::to_vec(&vault).unwrap()).is_err());
    }
    #[test]
    fn legacy_vault_data_and_import_preview() {
        let old=r#"{"categories":[],"accounts":[],"settings":{"auto_lock_seconds":180,"clipboard_seconds":25}}"#;
        let data: VaultData=serde_json::from_str(old).unwrap();
        assert!(data.trash.is_empty());
        let a=Account{id:"a".into(),category_id:String::new(),title:"Test".into(),username:String::new(),password:"secret".into(),website:String::new(),notes:String::new(),icon:"✦".into()};
        let source=VaultData{accounts:vec![a.clone()],..VaultData::default()};
        let mut target=VaultData{accounts:vec![a],..VaultData::default()};
        let p=preview_counts(&source,Some(&target),"merge",String::new());
        assert_eq!((p.added,p.unchanged,p.conflicts_as_new),(0,1,0));
        target.accounts[0].password="different".into();
        let p=preview_counts(&source,Some(&target),"merge",String::new());
        assert_eq!((p.added,p.unchanged,p.conflicts_as_new),(0,0,1));
        let p=preview_counts(&source,Some(&target),"replace",String::new());
        assert_eq!((p.added,p.replaced),(1,1));
        let (file,_,_)=make_file("a-very-long-test-master-password",&source).unwrap();
        assert_ne!(import_fingerprint(&file,None,"merge").unwrap(),import_fingerprint(&file,None,"replace").unwrap());
        assert_ne!(import_fingerprint(&file,None,"merge").unwrap(),import_fingerprint(&file,Some(&file),"merge").unwrap());
        let mut invalid=source.clone(); invalid.accounts[0].category_id="missing".into();
        assert!(validate_import(&invalid).is_err());
    }
    #[test]
    fn resource_roundtrip_legacy_and_import_counts() {
        let old=r#"{"categories":[],"accounts":[],"settings":{"auto_lock_seconds":180,"clipboard_seconds":25}}"#;
        let legacy:VaultData=serde_json::from_str(old).unwrap();
        assert!(legacy.resources.is_empty() && legacy.resource_trash.is_empty());
        let resource=Resource{id:"r1".into(),url:"https://github.com/example/repo".into(),title:"Repo".into(),
            description:"Description".into(),notes:"Private note".into(),tags:vec!["rust".into()],collection:"Work".into(),
            kind:"github".into(),folder_path:"D:\\Docs".into(),docs_url:String::new(),docs_checked:false,stars:Some(7),updated_at:String::new(),metadata_status:"ok".into()};
        assert!(valid_resource(&resource).is_ok());
        let source=VaultData{resources:vec![resource.clone()],..VaultData::default()};
        let (file,_,_)=make_file("a-very-long-test-master-password",&source).unwrap();
        let (_,restored)=unwrap_file(&file,"a-very-long-test-master-password",false).unwrap();
        assert_eq!(restored.resources[0],resource);
        assert_eq!(preview_counts(&source,Some(&source),"merge",String::new()).resources_unchanged,1);
        let mut changed=resource.clone();changed.notes="Other".into();
        let target=VaultData{resources:vec![changed],..VaultData::default()};
        assert_eq!(preview_counts(&source,Some(&target),"merge",String::new()).resources_conflicts_as_new,1);
        assert_eq!(preview_counts(&source,Some(&target),"replace",String::new()).resources_replaced,1);
        let mut invalid=source.clone();invalid.resources[0].url="file:///C:/secret".into();
        assert!(validate_import(&invalid).is_err());
        assert!(!public_address("127.0.0.1".parse().unwrap()));
        assert!(!public_address("192.168.1.2".parse().unwrap()));
        assert!(!public_address("::ffff:127.0.0.1".parse().unwrap()));
        assert!(metadata_response("https://127.0.0.1/private","text/html").is_err());
        assert_eq!(github_repo(&url::Url::parse("https://github.com/example/repo").unwrap()),Some(("example".into(),"repo".into())));
        assert!(discover_github_docs("a","b","main",|url|url.ends_with("/contents/docs")).ends_with("/tree/main/docs"));
        assert!(discover_github_docs("a","b","main",|url|url.ends_with("/readme")).ends_with("#readme"));
        assert!(discover_github_docs("a","b","main",|_|false).is_empty());
    }
    #[test]
    fn v12_encrypted_vault_upgrade_and_zip_restore() {
        use zip::{write::SimpleFileOptions, AesMode, CompressionMethod, ZipWriter};
        const MASTER: &str = "MassKey-test-master-2026";
        const ZIP_PASSWORD: &str = "MassKey-test-backup-2026";
        // The v1.2 payload schema has no trash or resource fields. Its file
        // format, Argon2 parameters, and AAD values match the v1.2.0 tag.
        let legacy = r##"{"categories":[{"id":"c1","name":"Work","icon":"W","color":"#9978fc"}],"accounts":[{"id":"a1","category_id":"c1","title":"Example","username":"test@example.invalid","password":"fixture-only-password","website":"https://example.invalid","notes":"fixture note","icon":"E"}],"settings":{"auto_lock_seconds":240,"clipboard_seconds":40}}"##;
        let (mut old_file, dek, recovery) = make_file(MASTER, &VaultData::default()).unwrap();
        old_file.payload = seal(&dek, legacy.as_bytes(), AAD_DATA).unwrap();
        let old_bytes = serde_json::to_vec(&old_file).unwrap();
        let parsed = parse_vault(&old_bytes).unwrap();
        let (unlocked_dek, old_data) = unwrap_file(&parsed, MASTER, false).unwrap();
        assert_eq!(old_data.accounts[0].password, "fixture-only-password");
        assert_eq!(old_data.settings.auto_lock_seconds, 240);
        assert!(old_data.resources.is_empty());
        assert_eq!(unwrap_file(&parsed, &recovery, true).unwrap().1.accounts.len(), 1);

        let mut upgraded = old_data.clone();
        upgraded.resources.push(Resource { id:"r1".into(), url:"https://example.org".into(),
            title:"Example resource".into(), description:String::new(), notes:String::new(),
            tags:vec!["test".into()], collection:"Work".into(), kind:"website".into(),
            folder_path:String::new(), docs_url:String::new(), docs_checked:false,
            stars:None, updated_at:String::new(), metadata_status:String::new() });
        let upgraded_file = encrypt_file(&unlocked_dek, &upgraded, parsed).unwrap();
        let (_, reopened) = unwrap_file(&upgraded_file, MASTER, false).unwrap();
        assert_eq!(serde_json::to_value(&reopened.accounts).unwrap(), serde_json::to_value(&old_data.accounts).unwrap());
        assert_eq!(reopened.settings.auto_lock_seconds, 240);
        assert_eq!(reopened.resources.len(), 1);

        let zip_path = std::env::temp_dir().join(format!("masskey-upgrade-{}.zip", Uuid::new_v4()));
        let result = (|| -> Result<(), String> {
            let mut writer = ZipWriter::new(File::create(&zip_path).map_err(err)?);
            let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored)
                .with_aes_encryption(AesMode::Aes256, ZIP_PASSWORD);
            writer.start_file("vault.nvault", options).map_err(err)?;
            writer.write_all(&serde_json::to_vec(&upgraded_file).map_err(err)?).map_err(err)?;
            writer.finish().map_err(err)?;
            let restored_file = read_zip(&zip_path, ZIP_PASSWORD)?;
            let (_, restored) = unwrap_file(&restored_file, MASTER, false)?;
            assert_eq!(serde_json::to_value(&restored.accounts).unwrap(), serde_json::to_value(&old_data.accounts).unwrap());
            assert_eq!(restored.resources, upgraded.resources);
            assert!(read_zip(&zip_path, "wrong-password").is_err());
            Ok(())
        })();
        let _ = fs::remove_file(&zip_path);
        result.unwrap();
    }
    #[test]
    fn failed_resource_delete_write_keeps_open_session() {
        let resource=Resource{id:"r".into(),url:"https://example.com".into(),title:"Example".into(),description:String::new(),
            notes:String::new(),tags:vec![],collection:String::new(),kind:"website".into(),folder_path:String::new(),
            docs_url:String::new(),docs_checked:false,stars:None,updated_at:String::new(),metadata_status:String::new()};
        let data=VaultData{resources:vec![resource],..VaultData::default()};
        let (file,dek,_)=make_file("MassKey-UAT-Only-2026!",&data).unwrap();
        let mut session=Session{dek,data,file,last_activity:Instant::now()};
        let mut next=session.data.clone();
        next.resource_trash.push(next.resources.remove(0));
        let missing=std::env::temp_dir().join(format!("masskey-missing-{}",Uuid::new_v4())).join("vault.nvault");
        assert!(persist_next_at(&missing,&mut session,next).is_err());
        assert_eq!(session.data.resources.len(),1);
        assert!(session.data.resource_trash.is_empty());
    }
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            #[cfg(windows)]
            windows_lock::start(app.state::<VaultState>().0.clone()).map_err(std::io::Error::other)?;
            Ok(())
        })
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Some(window)=app.get_webview_window("main") {
                let _=window.show(); let _=window.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(VaultState::default())
        .invoke_handler(tauri::generate_handler![
            vault_status, create_vault, unlock_vault, recover_vault, rotate_recovery, lock_vault,
            touch_session, snapshot, get_password, upsert_category, delete_category,
            upsert_account, delete_account, restore_account, trash_snapshot, permanently_delete_account, empty_trash, update_settings, change_master,
            generate_password, generate_similar_password, vault_health, open_account_website, export_zip, preview_import_zip, import_zip
            ,upsert_resource, delete_resource, restore_resource, permanently_delete_resource,
            open_resource_url, open_resource_folder, fetch_resource_metadata
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch MassKey Vault");
}
