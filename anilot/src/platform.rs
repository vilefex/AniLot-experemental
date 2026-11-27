//! Платформенные мелочи: каталог данных, открытие ссылок и внешних приложений.

use std::path::PathBuf;
use std::sync::OnceLock;

static DATA_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Android передаёт путь к приватному каталогу из `android_main`.
pub fn set_data_dir(path: PathBuf) {
    let _ = DATA_DIR.set(path);
}

pub fn data_dir() -> PathBuf {
    if let Some(p) = DATA_DIR.get() {
        return p.clone();
    }
    #[cfg(not(target_os = "android"))]
    {
        #[cfg(target_os = "ios")]
        {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join("Documents").join("AniLot");
            }
        }
        if let Some(d) = dirs::data_dir() {
            return d.join("AniLot");
        }
    }
    std::env::temp_dir().join("AniLot")
}

pub fn downloads_dir() -> PathBuf {
    data_dir().join("downloads")
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn is_mobile() -> bool {
    cfg!(any(target_os = "android", target_os = "ios"))
}

pub fn is_ios() -> bool {
    cfg!(target_os = "ios")
}

/// Открывает ссылку (или файл) во внешнем приложении системы.
pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux", target_os = "freebsd"))]
    {
        open::that_detached(url).map_err(|e| e.to_string())
    }
    #[cfg(target_os = "android")]
    {
        android_open(url)
    }
    #[cfg(target_os = "ios")]
    {
        ios_open(url)
    }
    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux",
        target_os = "freebsd",
        target_os = "android",
        target_os = "ios"
    )))]
    {
        Err(format!("Открытие ссылок не поддерживается на этой платформе: {url}"))
    }
}

#[cfg(target_os = "android")]
fn android_open(url: &str) -> Result<(), String> {
    use jni::objects::{JObject, JValue};
    let ctx = ndk_context::android_context();
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()) }.map_err(|e| e.to_string())?;
    let mut env = vm.attach_current_thread().map_err(|e| e.to_string())?;
    let activity = unsafe { JObject::from_raw(ctx.context().cast()) };
    let uri_class = env.find_class("android/net/Uri").map_err(|e| e.to_string())?;
    let jurl = env.new_string(url).map_err(|e| e.to_string())?;
    let uri = env
        .call_static_method(uri_class, "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&jurl)])
        .and_then(|v| v.l())
        .map_err(|e| e.to_string())?;
    let action = env.new_string("android.intent.action.VIEW").map_err(|e| e.to_string())?;
    let intent = env
        .new_object(
            "android/content/Intent",
            "(Ljava/lang/String;Landroid/net/Uri;)V",
            &[JValue::Object(&action), JValue::Object(&uri)],
        )
        .map_err(|e| e.to_string())?;
    // FLAG_ACTIVITY_NEW_TASK
    env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x1000_0000)])
        .map_err(|e| e.to_string())?;
    if let Err(e) = env.call_method(&activity, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(&intent)]) {
        let _ = env.exception_clear();
        return Err(e.to_string());
    }
    Ok(())
}

#[cfg(target_os = "ios")]
fn ios_open(url: &str) -> Result<(), String> {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use objc2_foundation::{NSDictionary, NSString, NSURL};
    unsafe {
        let ns = NSString::from_str(url);
        let nsurl = NSURL::URLWithString(&ns).ok_or_else(|| "Некорректная ссылка".to_string())?;
        let app: *mut AnyObject = msg_send![class!(UIApplication), sharedApplication];
        if app.is_null() {
            return Err("UIApplication недоступен".into());
        }
        let options: objc2::rc::Retained<NSDictionary<NSString, AnyObject>> = NSDictionary::new();
        let nil: *const AnyObject = std::ptr::null();
        let _: () = msg_send![app, openURL: &*nsurl, options: &*options, completionHandler: nil];
    }
    Ok(())
}

pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["Б", "КБ", "МБ", "ГБ"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{:.1} {}", v, UNITS[u])
    }
}
