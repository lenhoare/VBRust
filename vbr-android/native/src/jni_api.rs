//! JNI surface — only compiled when the `jni-bridge` feature is on (the APK).

#![allow(non_snake_case)]

#[cfg(feature = "jni-bridge")]
mod bridge {
    use crate::{complete, compile, hover, run, screen_dispatch, screen_start, screen_stop};
    use jni::objects::{JClass, JString};
    use jni::sys::jstring;
    use jni::JNIEnv;

    fn to_jstring(env: &mut JNIEnv, s: String) -> jstring {
        env.new_string(s)
            .map(|j| j.into_raw())
            .unwrap_or_else(|_| std::ptr::null_mut())
    }

    fn from_jstring(env: &mut JNIEnv, s: &JString) -> String {
        env.get_string(s)
            .map(|j| j.into())
            .unwrap_or_default()
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_setTccDir<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        path: JString<'local>,
    ) {
        crate::set_tcc_dir(from_jstring(&mut env, &path));
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_compile<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
    ) -> jstring {
        let src = from_jstring(&mut env, &source);
        let json = serde_json::to_string(&compile(&src)).unwrap_or_else(|e| {
            format!("{{\"has_errors\":true,\"code\":\"\",\"diagnostics\":[],\"blocked\":\"{e}\"}}")
        });
        to_jstring(&mut env, json)
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_run<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
    ) -> jstring {
        let src = from_jstring(&mut env, &source);
        let json = serde_json::to_string(&run(&src)).unwrap_or_else(|e| {
            format!(
                "{{\"stage\":\"compile\",\"success\":false,\"stdout\":\"\",\"stderr\":\"{e}\",\"code\":\"\",\"diagnostics\":[]}}"
            )
        });
        to_jstring(&mut env, json)
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_complete<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
        line: jni::sys::jint,
        col: jni::sys::jint,
    ) -> jstring {
        let src = from_jstring(&mut env, &source);
        let items = complete(&src, line as u32, col as u32);
        let json = serde_json::to_string(&items).unwrap_or_else(|_| "[]".into());
        to_jstring(&mut env, json)
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_hover<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
        line: jni::sys::jint,
        col: jni::sys::jint,
    ) -> jstring {
        let src = from_jstring(&mut env, &source);
        let text = hover(&src, line as u32, col as u32).unwrap_or_default();
        to_jstring(&mut env, text)
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_screenStart<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        source: JString<'local>,
    ) -> jstring {
        let src = from_jstring(&mut env, &source);
        to_jstring(&mut env, screen_start(&src))
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_screenDispatch<'local>(
        mut env: JNIEnv<'local>,
        _class: JClass<'local>,
        event: JString<'local>,
    ) -> jstring {
        let ev = from_jstring(&mut env, &event);
        to_jstring(&mut env, screen_dispatch(&ev))
    }

    #[no_mangle]
    pub extern "system" fn Java_dev_vbr_android_VbrNative_screenStop<'local>(
        _env: JNIEnv<'local>,
        _class: JClass<'local>,
    ) {
        screen_stop();
    }
}
