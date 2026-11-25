fn main() {
    println!("cargo:rerun-if-changed=../src-tauri/icons/icon.ico");
    println!("cargo:rerun-if-changed=windows-icon.rc");
    #[cfg(windows)]
    embed_resource::compile("windows-icon.rc", embed_resource::NONE)
        .manifest_required()
        .expect("Could not embed the Windows application icon");
}
